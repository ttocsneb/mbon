use maybe_async::maybe_async;

use std::{
    io::{self, SeekFrom},
    path::Path,
};

#[cfg(feature = "async-tokio")]
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};

#[cfg(feature = "sync")]
use std::thread::JoinHandle;
#[cfg(feature = "async-tokio")]
use tokio::task::JoinHandle;

#[cfg(feature = "sync")]
use std::{fs::File, io::Read, io::Seek};

use crate::{
    buffer::{FileBuffer, FileBufferOptions},
    concurrent::{ConcurrentEngineClient, ConcurrentEngineWrapper},
    data::{Data, PartialItem},
    errors::{MbonError, MbonResult},
    marks::Mark,
    stream::{Reader, Seeker},
};

/// Functions that are available in an Mbon engine reader
///
/// These are primarily functions that are for [crate::data] items to use for
/// parsing.
#[maybe_async]
pub trait MbonParserRead {
    async fn parse_mark(&mut self, location: u64) -> MbonResult<(Mark, u64)>;
    async fn parse_data(&mut self, mark: &Mark, location: u64) -> MbonResult<Data>;
    async fn parse_item(&mut self, location: u64) -> MbonResult<PartialItem>;
    async fn parse_item_n(
        &mut self,
        location: u64,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>>;
    async fn parse_data_n(&mut self, mark: &Mark, location: u64, n: usize)
        -> MbonResult<Vec<Data>>;
}

/// Mbon Engine
///
/// Manages I/O operations for an Mbon file.
pub struct Engine<F> {
    file: FileBuffer<F>,
}

#[maybe_async]
impl Engine<File> {
    /// Open an Mbon file in write mode
    pub async fn open_write(path: impl AsRef<Path>) -> io::Result<Self> {
        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .await?;
        Ok(Self::new(f))
    }

    /// Open an Mbon file in read mode
    pub async fn open_read(path: impl AsRef<Path>) -> io::Result<Self> {
        let f = File::options().read(true).open(path).await?;
        Ok(Self::new(f))
    }
}

impl<F: Reader + Seeker + Send + 'static> Engine<F> {
    /// Spawn a concurrent future which controls the engine and allow for
    /// multiple clients to concurrently make requests of the engine.
    ///
    /// This works in both synchronous and asynchronous mode.
    pub fn spawn_concurrent(self) -> (JoinHandle<io::Result<()>>, ConcurrentEngineClient) {
        let (wrapper, client) = ConcurrentEngineWrapper::new(self);
        let future = wrapper.spawn();
        (future, client)
    }
}

#[maybe_async]
impl<F> Engine<F>
where
    F: Reader + Seeker,
{
    /// Create a new engine from a file
    pub fn new(file: F) -> Self {
        Self {
            file: FileBufferOptions::new().build(file),
        }
    }

    /// Verify that the signature of the file is correct
    pub async fn verify_signature(&mut self) -> MbonResult<bool> {
        let file = &mut self.file;

        file.rewind().await?;
        let mut buf = [0u8; 8];
        file.read_exact(&mut buf).await?;
        const EXPECTED: [u8; 8] = [0xEE, 0x6D, 0x62, 0x6F, 0x6E, 0x0D, 0x0A, 0x00];
        Ok(buf == EXPECTED)
    }
}

#[maybe_async]
impl<F> MbonParserRead for Engine<F>
where
    F: Reader + Seeker,
{
    async fn parse_mark(&mut self, location: u64) -> MbonResult<(Mark, u64)> {
        let file = &mut self.file;

        file.seek(SeekFrom::Start(location)).await?;
        let (m, len) = Mark::parse(&mut *file).await?;
        Ok((m, location + len as u64))
    }

    async fn parse_data(&mut self, mark: &Mark, location: u64) -> MbonResult<Data> {
        let file = &mut self.file;

        file.seek(SeekFrom::Start(location)).await?;
        let data = Data::parse(&mut *file, mark).await?;
        Ok(data)
    }

    async fn parse_item(&mut self, location: u64) -> MbonResult<PartialItem> {
        let file = &mut self.file;

        file.seek(SeekFrom::Start(location)).await?;
        let (m, _) = Mark::parse(&mut *file).await?;
        let mut item = PartialItem::new(m, location);
        item.parse_data(&mut *file).await?;
        Ok(item)
    }

    async fn parse_item_n(
        &mut self,
        location: u64,
        count: Option<usize>,
        bytes: u64,
        parse_data: bool,
    ) -> MbonResult<Vec<PartialItem>> {
        let file = &mut self.file;

        let mut items = Vec::new();
        let mut read = 0;
        let mut pos = file.seek(SeekFrom::Start(location)).await?;

        while count.map(|count| items.len() < count).unwrap_or(true) && read < bytes {
            let (m, _) = Mark::parse(&mut *file).await?;
            let mut item = PartialItem::new(m, pos);
            if parse_data {
                item.parse_data(&mut *file).await?;
            }

            let len = item.mark.total_len();
            read += len;

            pos = file.seek(SeekFrom::Start(pos + len)).await?;
            items.push(item);
        }

        if read > bytes {
            return Err(MbonError::InvalidMark);
        }

        Ok(items)
    }

    async fn parse_data_n(
        &mut self,
        mark: &Mark,
        location: u64,
        n: usize,
    ) -> MbonResult<Vec<Data>> {
        let file = &mut self.file;

        let mut items = Vec::new();
        let start = file.seek(SeekFrom::Start(location)).await?;

        let len = mark.data_len();

        for i in 0..n {
            file.seek(SeekFrom::Start(start + (len * i as u64))).await?;
            let data = Data::parse(&mut *file, mark).await?;
            items.push(data);
        }

        Ok(items)
    }
}
