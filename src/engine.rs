use maybe_async::maybe_async;

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

#[cfg(feature = "async")]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "async-tokio")]
use tokio::task::{spawn_blocking, JoinHandle};

#[cfg(feature = "sync")]
use std::thread::JoinHandle;

use crate::{
    buffer::{FileBuffer, FileBufferBuilder},
    concurrent::{ConcurrentEngineClient, ConcurrentEngineWrapper},
    data::{Data, PartialItem},
    errors::{MbonError, MbonResult},
    marks::Mark,
};

/// Functions that are available in an Mbon engine reader
///
/// These are primarily functions that are for [crate::data] items to use for
/// parsing.
///
/// The specific functions in this trait need to be narrowed down a bit more.
///
/// There should be functions that are specialized to the different types of
/// items that are available.
///
/// I would also like the idea to be able to parse an item in its entirety if
/// requested. Currently, it is setup so that each item that is parsed is only
/// partially parsed.
#[maybe_async]
pub trait MbonParserRead {
    async fn parse_mark(&mut self, location: SeekFrom) -> MbonResult<(Mark, u64)>;
    async fn parse_data(&mut self, mark: &Mark, location: SeekFrom) -> MbonResult<(Data, u64)>;
    async fn parse_item(&mut self, location: SeekFrom) -> MbonResult<PartialItem>;
    async fn parse_item_n(
        &mut self,
        location: SeekFrom,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>>;
    async fn parse_data_n(
        &mut self,
        mark: &Mark,
        location: SeekFrom,
        n: usize,
    ) -> MbonResult<Vec<Data>>;
}

#[cfg(feature = "sync")]
type Reader<F> = FileBuffer<F>;
#[cfg(feature = "async")]
type Reader<F> = Arc<Mutex<FileBuffer<F>>>;

/// Mbon Engine
///
/// Manages I/O operations for an Mbon file.
pub struct Engine<F> {
    file: Reader<F>,
}

#[cfg(feature = "async")]
impl<F> Clone for Engine<F> {
    fn clone(&self) -> Self {
        Engine {
            file: self.file.clone(),
        }
    }
}

impl Engine<File> {
    /// Open an Mbon file in write mode
    pub fn open_write(path: impl AsRef<Path>) -> io::Result<Self> {
        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        Ok(Self::new(f))
    }

    /// Open an Mbon file in read mode
    pub fn open_read(path: impl AsRef<Path>) -> io::Result<Self> {
        let f = File::options().read(true).open(path)?;
        Ok(Self::new(f))
    }
}

#[cfg(feature = "async")]
impl<F> Engine<F> {
    #[inline]
    fn get_file(&mut self) -> MutexGuard<FileBuffer<F>> {
        self.file.lock().unwrap()
    }

    #[inline]
    fn new_file(f: FileBuffer<F>) -> Arc<Mutex<FileBuffer<F>>> {
        Arc::new(Mutex::new(f))
    }
}

#[cfg(feature = "sync")]
impl<F> Engine<F> {
    #[inline]
    fn get_file(&mut self) -> &mut FileBuffer<F> {
        &mut self.file
    }
    #[inline]
    fn new_file(f: FileBuffer<F>) -> FileBuffer<F> {
        f
    }
}

impl<F> Engine<F>
where
    F: Read + Seek + Send + 'static,
{
    /// Spawn a new thread to process engine requests
    ///
    /// This will return a [JoinHandle] for the new thread and an
    /// [ConcurrentEngineClient] which will allow for multiple concurrent
    /// requests to the engine.
    pub fn spawn_client_thread(self) -> (JoinHandle<io::Result<()>>, ConcurrentEngineClient) {
        let (wrapper, client) = ConcurrentEngineWrapper::new(self);
        let handle = wrapper.spawn();
        (handle, client)
    }
}

impl<F> Engine<F>
where
    F: Read + Seek,
{
    /// Create a new engine from a file
    pub fn new(file: F) -> Self {
        Self {
            file: Self::new_file(FileBufferBuilder::new().build_sync(file)),
        }
    }

    /// Synchronously verify the signature
    pub fn verify_signature_sync(&mut self) -> MbonResult<bool> {
        #[allow(unused_mut)]
        let mut file = self.get_file();
        file.rewind()?;
        let mut buf = [0u8; 8];
        file.read_exact(&mut buf)?;
        const EXPECTED: [u8; 8] = [0xEE, 0x6D, 0x62, 0x6F, 0x6E, 0x0D, 0x0A, 0x00];
        if buf != EXPECTED {
            return Ok(false);
        }
        Ok(true)
    }

    /// Synchronously parse a mark at the given location
    pub fn parse_mark_sync(&mut self, location: SeekFrom) -> MbonResult<(Mark, u64)> {
        #[allow(unused_mut)]
        let mut file = self.get_file();
        let pos = file.seek(location)?;
        let (m, _) = Mark::parse(&mut *file)?;
        Ok((m, pos))
    }

    /// Synchronously parse an item at the given location
    pub fn parse_item_sync(&mut self, location: SeekFrom) -> MbonResult<PartialItem> {
        #[allow(unused_mut)]
        let mut file = self.get_file();
        let pos = file.seek(location)?;
        let (m, _) = Mark::parse(&mut *file)?;
        let mut item = PartialItem::new(m, pos);
        item.parse_data(&mut *file)?;
        Ok(item)
    }

    /// Synchronously parse several items in a sequence
    pub fn parse_item_n_sync(
        &mut self,
        location: SeekFrom,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>> {
        #[allow(unused_mut)]
        let mut file = self.get_file();

        let mut items = Vec::new();
        let mut read = 0;
        let mut pos = file.seek(location)?;

        while count.map(|count| items.len() < count).unwrap_or(true) && read < bytes {
            let (m, _) = Mark::parse(&mut *file)?;
            let mut item = PartialItem::new(m, pos);
            if parse_data {
                item.parse_data(&mut *file)?;
            }

            let len = item.mark.total_len();
            read += len;

            pos = file.seek(SeekFrom::Start(pos + len))?;
            items.push(item);
        }

        if read > bytes {
            return Err(MbonError::InvalidMark);
        }

        Ok(items)
    }

    pub fn parse_data_sync(&mut self, mark: &Mark, location: SeekFrom) -> MbonResult<(Data, u64)> {
        #[allow(unused_mut)]
        let mut file = self.get_file();
        let pos = file.seek(location)?;
        let data = Data::parse(&mut *file, mark)?;
        Ok((data, pos))
    }

    pub fn parse_data_n_sync(
        &mut self,
        mark: &Mark,
        location: SeekFrom,
        n: usize,
    ) -> MbonResult<Vec<Data>> {
        #[allow(unused_mut)]
        let mut file = self.get_file();

        let mut items = Vec::new();
        let start = file.seek(location)?;

        let len = mark.data_len();

        for i in 0..n {
            file.seek(SeekFrom::Start(start + (len * i as u64)))?;
            let data = Data::parse(&mut *file, mark)?;
            items.push(data);
        }

        Ok(items)
    }
}

#[cfg(feature = "async-tokio")]
macro_rules! mbon_parser_impl {
    ($self:ident, $s:ident => $expr:expr) => {{
        let mut $s = $self.clone();
        spawn_blocking(move || $expr).await.unwrap()
    }};
    ($self:ident, ($($to_clone:ident),*) $s:ident => $expr:expr) => {{
        let mut $s = $self.clone();
        $(let $to_clone = $to_clone.clone());*;
        spawn_blocking(move || $expr).await.unwrap()
    }};
}

#[cfg(feature = "sync")]
macro_rules! mbon_parser_impl {
    ($self:ident, $s:ident => $expr:expr) => {{
        let $s = $self;
        $expr
    }};
    ($self:ident, ($($to_clone:ident),*) $s:ident => $expr:expr) => {{
        let $s = $self;
        $expr
    }};
}

#[maybe_async]
impl<F> MbonParserRead for Engine<F>
where
    F: Read + Seek + Send + 'static,
{
    async fn parse_mark(&mut self, location: SeekFrom) -> MbonResult<(Mark, u64)> {
        mbon_parser_impl!(self, s => s.parse_mark_sync(location))
    }

    async fn parse_data(&mut self, mark: &Mark, location: SeekFrom) -> MbonResult<(Data, u64)> {
        mbon_parser_impl!(self, (mark) s => s.parse_data_sync(&mark, location))
    }

    async fn parse_item(&mut self, location: SeekFrom) -> MbonResult<PartialItem> {
        mbon_parser_impl!(self, s => s.parse_item_sync(location))
    }

    async fn parse_item_n(
        &mut self,
        location: SeekFrom,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>> {
        mbon_parser_impl!(self, s => s.parse_item_n_sync(location, count, bytes, parse_data))
    }

    async fn parse_data_n(
        &mut self,
        mark: &Mark,
        location: SeekFrom,
        n: usize,
    ) -> MbonResult<Vec<Data>> {
        mbon_parser_impl!(self, (mark) s => s.parse_data_n_sync(&mark, location, n))
    }
}
