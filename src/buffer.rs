//! Contains [FileBuffer], which is a wrapper for files.
//!
//! There is now an asynchronous implementation of [FileBuffer]:
//! [FileBufferAsync]. It has not been tested yet, but I like they way it is
//! implemented and should be implemented in a similar way for [FileBuffer]
//! (Rather than doing work upfront, FileBuffer should instead perform actions
//! as it goes).
//!
//! The Buffer helper struct also needs to be majorly cleaned up. I'm tired,
//! good night. 😴

use std::{
    collections::{BTreeSet, BinaryHeap, HashMap},
    mem,
    ops::Range,
    pin::Pin,
    task::{Context, Poll},
};

use std::io::{self, Read, Seek, SeekFrom, Write};

use enum_as_inner::EnumAsInner;
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

struct Block {
    data: Vec<u8>,
    access: u64,
}

struct Buffer {
    blocks: HashMap<u64, Block>,
    modified: BTreeSet<u64>,
    block_size: u64,
    max_blocks: usize,
    ideal_blocks: usize,
    cursor: u64,
    access_count: u64,
}

// This macro is needed because wrapping it in a function causes issues with the
// borrow checker. (only access_count is modified)
macro_rules! get_block {
    ($self:ident, $block:expr) => {{
        if let Some(block) = $self.blocks.get_mut(&$block) {
            block.access = $self.access_count;
            $self.access_count += 1;
            Some(&block.data)
        } else {
            None
        }
    }};
    (mut $self:ident, $block:expr) => {{
        if let Some(block) = $self.blocks.get_mut(&$block) {
            block.access = $self.access_count;
            $self.access_count += 1;
            Some(&mut block.data)
        } else {
            None
        }
    }};
}

impl Buffer {
    fn purge_least_recently_used(&mut self) {
        let n_blocks = self.blocks.len() - self.ideal_blocks;
        let mut to_delete = BinaryHeap::new();

        for (k, v) in &self.blocks {
            if self.modified.contains(&k) {
                // Don't try to clean modified blocks
                continue;
            }
            if to_delete.len() < n_blocks {
                to_delete.push((v.access, *k));
            } else if let Some((access, _)) = to_delete.peek() {
                if v.access <= *access {
                    to_delete.push((v.access, *k));
                }
                if to_delete.len() > n_blocks {
                    to_delete.pop();
                }
            }
        }

        for (_, k) in to_delete {
            self.blocks.remove(&k);
        }
    }

    /// Read from the buffer.
    ///
    /// The buffer must be pre-loaded in order for this to work.
    fn read_buffered(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut offset = 0;
        let mut sect = self.cursor / self.block_size;
        let mut sect_index = self.cursor % self.block_size;
        let mut block = get_block!(self, sect);

        while let Some(buffer) = block {
            let buffer = &buffer[sect_index as usize..];
            let b = &mut buf[offset..];
            let to_read = buffer.len().min(b.len());

            let b = &mut b[..to_read];
            b.copy_from_slice(&buffer[..to_read]);
            offset += to_read;
            self.cursor += to_read as u64;
            if offset == buf.len() {
                break;
            }

            sect += 1;
            sect_index = 0;
            block = get_block!(self, sect);
        }

        if self.blocks.len() > self.max_blocks {
            self.purge_least_recently_used();
        }

        Ok(offset)
    }

    fn get_next_block(&mut self) -> Result<(usize, &mut Vec<u8>), (usize, u64)> {
        let block = self.cursor / self.block_size;
        let offset = (self.cursor % self.block_size) as usize;
        match get_block!(mut self, block) {
            Some(data) => Ok((offset, data)),
            None => Err((offset, block)),
        }
    }

    fn get_next_block_modify(&mut self) -> Result<(usize, &mut Vec<u8>), (usize, u64)> {
        let block = self.cursor / self.block_size;
        let offset = (self.cursor % self.block_size) as usize;
        match get_block!(mut self, block) {
            Some(data) => {
                self.modified.insert(block);
                Ok((offset, data))
            }
            None => Err((offset, block)),
        }
    }

    fn get_next_modified_block(&self) -> Option<u64> {
        self.modified.first().copied()
    }

    fn iter_blocks(&self, position: u64, len: u64) -> Range<u64> {
        let end = position + len;
        let block = position / self.block_size;
        let end_block = (end + self.block_size - 1) / self.block_size;
        let num_blocks = end_block - block;

        block..block + num_blocks
    }

    fn to_load(&self, position: u64, len: u64) -> Vec<u64> {
        let mut to_load = Vec::new();

        for sect in self.iter_blocks(position, len) {
            if !self.blocks.contains_key(&sect) {
                to_load.push(sect);
            }
        }

        to_load
    }

    /// Write to the internal buffer
    ///
    /// Any pre-existing blocks must already be loaded for this to work.
    ///
    /// No writes to the disk will occur
    fn write(&mut self, buf: &[u8]) -> usize {
        let mut offset = 0;
        let mut sect = self.cursor / self.block_size;
        let mut sect_index = self.cursor % self.block_size;
        let block_size = self.block_size;
        let mut block = get_block!(mut self, sect);

        while let Some(buffer) = block {
            let write = &mut buffer[sect_index as usize..];
            let read = &buf[offset..];
            let to_write = write.len().min(read.len());

            let write = &mut write[..to_write];
            write.copy_from_slice(&read[..to_write]);
            self.modified.insert(sect);

            self.cursor += to_write as u64;
            offset += to_write;

            if offset == buf.len() {
                break;
            }

            if (buffer.len() as u64) < block_size {
                // If the block isn't a full block, write to the end of the block

                let mut write = vec![0u8; block_size as usize - buffer.len()];
                let read = &buf[offset..];
                let to_write = write.len().min(read.len());

                let write = &mut write[..to_write];
                write.copy_from_slice(&read[..to_write]);
                buffer.extend_from_slice(write);
                self.cursor += to_write as u64;

                offset += to_write;

                if offset == buf.len() {
                    break;
                }
            }

            sect += 1;
            sect_index = 0;
            block = get_block!(mut self, sect);
        }

        while offset < buf.len() {
            // There are new blocks to write

            let read = &buf[offset..];
            let max_write = self.max_blocks - sect_index as usize;
            let to_write = max_write.min(read.len());

            let mut buffer = vec![0u8; sect_index as usize + to_write];

            let write = &mut buffer[sect_index as usize..];
            write.copy_from_slice(read);

            self.cursor += to_write as u64;
            offset += to_write;

            self.blocks.insert(
                sect,
                Block {
                    data: buffer,
                    access: self.access_count,
                },
            );
            self.access_count += 1;
            self.modified.insert(sect);

            sect += 1;
            sect_index = 0;
        }

        offset
    }

    fn take_modified(&mut self) -> Vec<u64> {
        let mut modified: Vec<_> = mem::take(&mut self.modified).into_iter().collect();
        modified.sort_unstable();
        modified
    }

    fn get_block_mut(&mut self, block: u64) -> Option<&mut Vec<u8>> {
        get_block!(mut self, block)
    }

    fn is_full(&self) -> bool {
        self.blocks.len() > self.max_blocks
    }

    fn insert(&mut self, block: u64, data: Vec<u8>) {
        self.blocks.insert(
            block,
            Block {
                data,
                access: self.access_count,
            },
        );
        self.access_count += 1;
    }

    #[inline]
    fn mark_modified(&mut self, block: u64) {
        self.modified.insert(block);
    }

    #[inline]
    fn mark_clean(&mut self, block: u64) {
        self.modified.remove(&block);
    }
}

/// A wrapper for files which holds a buffer for the file.
///
/// This buffer can hold the entire file in memory and has an option to limit
/// how much data is stored in memory (the default limit is 1GiB).
///
/// Reads and writes are performed in blocks (the default block size is 512
/// bytes).
///
/// This struct is designed to work with simultaneous read/write operations.
///
/// No writes occur to the underlying file until either flush is called, or the
/// cache limit has been met.
pub struct FileBuffer<F> {
    buffer: Buffer,
    file: F,
}

/// Builder for [FileBuffer].
pub struct FileBufferBuilder {
    block_size: Option<u64>,
    max_blocks: Option<usize>,
    ideal_blocks: Option<usize>,
    max_cache: Option<u64>,
    ideal_cache: Option<u64>,
}

impl FileBufferBuilder {
    pub fn new() -> Self {
        FileBufferBuilder {
            block_size: None,
            max_blocks: None,
            max_cache: None,
            ideal_cache: None,
            ideal_blocks: None,
        }
    }

    /// Set the number of bytes in a block.
    ///
    /// The default is 512 bytes.
    pub fn with_block_size(mut self, block_size: u64) -> Self {
        self.block_size = Some(block_size);
        self
    }

    /// The maximum number of blocks to have in the cache.
    ///
    /// This sets the same value as [Self::with_max_cache], but in a different
    /// unit
    ///
    /// The default value is 1GiB.
    ///
    /// Note that during a single read, the cache may become larger than the
    /// maximum cache for the duration of the read.
    pub fn with_max_blocks(mut self, max_blocks: usize) -> Self {
        self.max_blocks = Some(max_blocks);
        self.max_cache = None;
        self
    }

    /// The maximum number of bytes to have in the cache.
    ///
    /// This sets the same value as [Self::with_max_blocks], but in a different unit
    ///
    /// The default value is 1GiB.
    ///
    /// Note that during a single read, the cache may become larger than the
    /// maximum cache for the duration of the read.
    pub fn with_max_cache(mut self, max_cache: u64) -> Self {
        self.max_cache = Some(max_cache);
        self.max_blocks = None;
        self
    }

    /// The number of blocks to reduce the cache to when the cache has filled up.
    ///
    /// This sets the same value as [Self::with_ideal_cache], but in a different
    /// unit
    ///
    /// The default value is `max_cache - (1MiB, 1KiB, or max_cache / 5
    /// /* Which ever is the largest value smaller than max_cache*/)`.
    pub fn with_ideal_blocks(mut self, max_blocks: usize) -> Self {
        self.ideal_blocks = Some(max_blocks);
        self.ideal_cache = None;
        self
    }

    /// The number of bytes to reduce the cache to when the cache has filled up.
    ///
    /// This sets the same value as [Self::with_ideal_blocks], but in a
    /// different unit
    ///
    /// The default value is `max_cache - (1MiB, 1KiB, or max_cache / 5
    /// /* Which ever is the largest value smaller than max_cache*/)`.
    pub fn with_ideal_cache(mut self, max_cache: u64) -> Self {
        self.ideal_cache = Some(max_cache);
        self.ideal_blocks = None;
        self
    }

    fn build(self) -> Buffer {
        let block_size = self.block_size.unwrap_or(512);
        let max_blocks = self
            .max_blocks
            .unwrap_or_else(|| (self.max_cache.unwrap_or(1_073_741_824) / block_size) as usize);
        let ideal_blocks = self
            .ideal_blocks
            .or_else(|| self.ideal_cache.map(|cache| (cache / block_size) as usize))
            .unwrap_or_else(|| {
                let mut blocks = (1_048_576 / block_size) as usize;
                if blocks > max_blocks {
                    blocks = (1024 / block_size) as usize;
                }
                if blocks > max_blocks {
                    blocks = max_blocks / 5;
                }

                max_blocks - blocks
            });
        Buffer {
            blocks: HashMap::new(),
            modified: BTreeSet::new(),
            cursor: 0,
            block_size,
            max_blocks,
            ideal_blocks,
            access_count: 0,
        }
    }

    /// Create the FileBuffer object
    pub fn build_sync<F>(self, f: F) -> FileBuffer<F> {
        let buffer = self.build();

        FileBuffer { file: f, buffer }
    }

    pub fn build_async<F>(self, f: F) -> FileBufferAsync<F> {
        let buffer = self.build();

        FileBufferAsync {
            file: f,
            buffer,
            cursor: None,
            state: AsyncFileBufState::default(),
        }
    }
}

impl<F> FileBuffer<F>
where
    F: Write + Seek,
{
    fn flush_blocks(&mut self) -> io::Result<()> {
        let modified = self.buffer.take_modified();

        let mut position = match modified.first() {
            Some(sect) => self
                .file
                .seek(SeekFrom::Start(sect * self.buffer.block_size))?,
            None => self.file.seek(SeekFrom::Current(0))?,
        };
        let block_size = self.buffer.block_size;

        for block in modified {
            let buf = match self.buffer.get_block_mut(block) {
                Some(b) => b,
                None => continue,
            };
            let pos = block * block_size;
            if position != pos {
                self.file.seek(SeekFrom::Start(pos))?;
                position = pos;
            }

            self.file.write_all(buf.as_slice())?;
            position += buf.len() as u64;
        }
        self.file.flush()?;

        if self.buffer.is_full() {
            self.buffer.purge_least_recently_used();
        }

        Ok(())
    }

    /// Clear the cache
    ///
    /// If there are any modified changes, they will be written to disk before
    /// clearing the cache.
    pub fn clear_cache(&mut self) -> io::Result<()> {
        self.flush_blocks()?;
        self.buffer.blocks.clear();
        Ok(())
    }
}

impl<F> FileBuffer<F>
where
    F: Read + Seek,
{
    fn load_blocks(&mut self, position: u64, len: u64) -> io::Result<()> {
        let to_load = self.buffer.to_load(position, len);

        let mut position = self.file.seek(SeekFrom::Current(0))?;
        for sect in to_load {
            let pos = sect * self.buffer.block_size;
            if position != pos {
                self.file.seek(SeekFrom::Start(pos))?;
                position = pos;
            }
            let mut buf = vec![0u8; self.buffer.block_size as usize];
            let mut offset = 0;
            let mut eof = false;

            while offset < buf.len() {
                let read = match self.file.read(&mut buf[offset..]) {
                    Ok(n) => n,
                    Err(err) => match err.kind() {
                        io::ErrorKind::Interrupted => {
                            continue;
                        }
                        _ => return Err(err),
                    },
                };
                if read == 0 {
                    eof = true;
                    break;
                }
                position += read as u64;
                offset += read;
            }
            for i in (offset..buf.len()).rev() {
                buf.remove(i);
            }

            self.buffer.insert(sect, buf);

            if eof {
                break;
            }
        }

        Ok(())
    }
}

impl<F> Read for FileBuffer<F>
where
    F: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.load_blocks(self.buffer.cursor, buf.len() as u64)?;
        self.buffer.read_buffered(buf)
    }
}

impl<F> Write for FileBuffer<F>
where
    F: Read + Write + Seek,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.load_blocks(self.buffer.cursor, buf.len() as u64)?;

        let offset = self.buffer.write(buf);

        if self.buffer.is_full() {
            self.flush_blocks()?;
        }

        Ok(offset)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_blocks()
    }
}

impl<F> Seek for FileBuffer<F>
where
    F: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => self.buffer.cursor = p,
            SeekFrom::End(_) => {
                self.buffer.cursor = self.file.seek(pos)?;
            }
            SeekFrom::Current(o) => {
                self.buffer.cursor = self.buffer.cursor.checked_add_signed(o).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot seek to a negative position",
                    )
                })?
            }
        }
        Ok(self.buffer.cursor)
    }
}

#[derive(EnumAsInner)]
enum AsyncFileBufState {
    Normal,
    StartSeek(SeekFrom),
    Seeking,
    Reading {
        block: u64,
        buf: Vec<u8>,
        read: usize,
    },
    Writing {
        block: u64,
        buf: Vec<u8>,
        written: usize,
    },
    Closing,
}

impl Default for AsyncFileBufState {
    fn default() -> Self {
        Self::Normal
    }
}

#[pin_project]
pub struct FileBufferAsync<F> {
    buffer: Buffer,
    #[pin]
    file: F,
    cursor: Option<u64>,
    state: AsyncFileBufState,
}
impl<F: AsyncRead + AsyncSeek> FileBufferAsync<F> {
    fn internal_poll_read_block(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        block: u64,
    ) -> Poll<io::Result<()>> {
        let state = mem::take(self.as_mut().project().state);
        let block_size = self.buffer.block_size;
        match state {
            AsyncFileBufState::Normal => {
                if self
                    .as_mut()
                    .internal_cursor_try_seek(cx, SeekFrom::Start(block * block_size))?
                    .is_pending()
                {
                    return Poll::Pending;
                }

                let me = self.as_mut().project();
                *me.state = AsyncFileBufState::Reading {
                    block,
                    buf: vec![0u8; block_size as usize],
                    read: 0,
                };
                self.internal_poll_read_block(cx, block)
            }
            AsyncFileBufState::Reading {
                block,
                mut buf,
                read,
            } => {
                let me = self.as_mut().project();
                debug_assert_eq!(*me.cursor, Some(block * block_size + read as u64));

                let mut b = ReadBuf::new(buf.as_mut_slice());
                b.set_filled(read);
                if me.file.poll_read(cx, &mut b)?.is_pending() {
                    return Poll::Pending;
                }
                let read = b.filled().len();
                *me.cursor = Some(block * me.buffer.block_size + read as u64);
                if b.remaining() == 0 {
                    me.buffer.insert(block, buf);
                    return Poll::Ready(Ok(()));
                }
                *me.state = AsyncFileBufState::Reading { block, buf, read };
                self.internal_poll_read_block(cx, block)
            }
            AsyncFileBufState::Writing { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Writing State",
            ))),
            AsyncFileBufState::Closing => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "Closed")))
            }
            state => {
                let me = self.as_mut().project();
                *me.state = state;

                let poll = self.as_mut().internal_cursor_poll_complete(cx)?;
                if poll.is_ready() {
                    return self.internal_poll_read_block(cx, block);
                }
                Poll::Pending
            }
        }
    }
}

impl<F: AsyncRead + AsyncSeek> AsyncRead for FileBufferAsync<F> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        while buf.remaining() > 0 {
            let me = self.as_mut().project();
            match me.buffer.get_next_block() {
                Ok((offset, block)) => {
                    let block = &block[offset..];
                    let to_read = block.len().min(buf.remaining());
                    buf.put_slice(&block[..to_read]);
                    me.buffer.cursor += to_read as u64;
                    continue;
                }
                Err((_offset, block)) => {
                    if self
                        .as_mut()
                        .internal_poll_read_block(cx, block)?
                        .is_pending()
                    {
                        return Poll::Pending;
                    }
                }
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl<F: AsyncSeek> FileBufferAsync<F> {
    fn internal_cursor_try_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        position: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        if self.state.is_normal() {
            self.as_mut().internal_cursor_start_seek(position)?;
        }
        self.internal_cursor_poll_complete(cx)
    }

    fn internal_cursor_start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        let me = self.project();
        *me.state = AsyncFileBufState::StartSeek(position);
        Ok(())
    }

    fn internal_cursor_poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<u64>> {
        let state = mem::take(self.as_mut().project().state);
        match state {
            AsyncFileBufState::Normal => Poll::Ready(Ok(0)),
            AsyncFileBufState::StartSeek(seek) => match seek {
                SeekFrom::Start(pos) => {
                    let me = self.as_mut().project();
                    if let Some(actual) = me.cursor {
                        if *actual == pos {
                            return Poll::Ready(Ok(*actual));
                        }
                    }
                    me.file.start_seek(SeekFrom::Start(pos))?;
                    let me = self.as_mut().project();
                    let poll = me.file.poll_complete(cx)?;
                    if let Poll::Ready(pos) = poll {
                        *me.cursor = Some(pos);
                        return Poll::Ready(Ok(pos));
                    }
                    *me.state = AsyncFileBufState::Seeking;
                    Poll::Pending
                }
                seek => {
                    let me = self.as_mut().project();
                    me.file.start_seek(seek)?;
                    let me = self.as_mut().project();
                    let poll = me.file.poll_complete(cx)?;
                    if let Poll::Ready(pos) = poll {
                        *me.cursor = Some(pos);
                        return Poll::Ready(Ok(pos));
                    }
                    *me.state = AsyncFileBufState::Seeking;
                    Poll::Pending
                }
            },
            AsyncFileBufState::Seeking => {
                let me = self.as_mut().project();
                let poll = me.file.poll_complete(cx)?;
                if let Poll::Ready(pos) = poll {
                    *me.cursor = Some(pos);
                    return Poll::Ready(Ok(pos));
                }
                *me.state = AsyncFileBufState::Seeking;
                Poll::Pending
            }
            AsyncFileBufState::Reading { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Reading State",
            ))),
            AsyncFileBufState::Writing { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Writing State",
            ))),
            AsyncFileBufState::Closing => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "Closed")))
            }
        }
    }
}

impl<F: AsyncSeek> AsyncSeek for FileBufferAsync<F> {
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        let me = self.project();
        *me.state = AsyncFileBufState::StartSeek(position);
        Ok(())
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        let me = self.as_mut().project();
        let state = mem::take(me.state);
        match state {
            AsyncFileBufState::Normal => Poll::Ready(Ok(me.buffer.cursor)),
            AsyncFileBufState::StartSeek(SeekFrom::Start(position)) => {
                me.buffer.cursor = position;
                Poll::Ready(Ok(me.buffer.cursor))
            }
            AsyncFileBufState::StartSeek(SeekFrom::Current(offset)) => {
                me.buffer.cursor = match me.buffer.cursor.checked_add_signed(offset) {
                    Some(v) => v,
                    None => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Cannot seek to a negative position",
                        )))
                    }
                };
                Poll::Ready(Ok(me.buffer.cursor))
            }
            AsyncFileBufState::StartSeek(seek) => {
                me.file.start_seek(seek)?;
                let me = self.as_mut().project();
                let poll = me.file.poll_complete(cx)?;
                if let Poll::Ready(pos) = poll {
                    me.buffer.cursor = pos;
                    *me.cursor = Some(pos);
                    return Poll::Ready(Ok(me.buffer.cursor));
                }
                *me.state = AsyncFileBufState::Seeking;
                Poll::Pending
            }
            AsyncFileBufState::Seeking => {
                let poll = me.file.poll_complete(cx)?;
                if let Poll::Ready(pos) = poll {
                    me.buffer.cursor = pos;
                    *me.cursor = Some(pos);
                    return Poll::Ready(Ok(me.buffer.cursor));
                }
                *me.state = AsyncFileBufState::Seeking;
                Poll::Pending
            }
            AsyncFileBufState::Reading { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Reading State",
            ))),
            AsyncFileBufState::Writing { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Writing State",
            ))),
            AsyncFileBufState::Closing => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "Closed")))
            }
        }
    }
}

impl<F: AsyncWrite + AsyncSeek> FileBufferAsync<F> {
    fn internal_start_write_block(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        block: u64,
    ) -> Poll<io::Result<()>> {
        let state = mem::take(self.as_mut().project().state);
        let block_size = self.buffer.block_size;
        match state {
            AsyncFileBufState::Normal => {
                if self
                    .as_mut()
                    .internal_cursor_try_seek(cx, SeekFrom::Start(block * block_size))?
                    .is_pending()
                {
                    return Poll::Pending;
                }

                let me = self.as_mut().project();
                let data = me.buffer.blocks.get(&block).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "Block does not exist")
                })?;
                *me.state = AsyncFileBufState::Writing {
                    block,
                    buf: data.data.clone(),
                    written: 0,
                };
                self.internal_poll_write_block(cx)
            }
            AsyncFileBufState::Reading { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Reading State",
            ))),
            AsyncFileBufState::Writing { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Writing State",
            ))),
            AsyncFileBufState::Closing => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "Closed")))
            }
            state => {
                let me = self.as_mut().project();
                *me.state = state;

                let poll = self.as_mut().internal_cursor_poll_complete(cx)?;
                if poll.is_ready() {
                    return self.internal_poll_write_block(cx);
                }
                Poll::Pending
            }
        }
    }
    fn internal_poll_write_block(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let state = mem::take(self.as_mut().project().state);
        let block_size = self.buffer.block_size;

        match state {
            AsyncFileBufState::Writing {
                block,
                buf,
                written,
            } => {
                let me = self.as_mut().project();
                debug_assert_eq!(*me.cursor, Some(block * block_size + written as u64));

                let poll = me.file.poll_write(cx, &buf[written..])?;
                if let Poll::Ready(w) = poll {
                    let written = written + w;
                    *me.cursor = Some(block * block_size + written as u64);
                    if written == buf.len() {
                        me.buffer.mark_clean(block);
                        return Poll::Ready(Ok(()));
                    }

                    *me.state = AsyncFileBufState::Writing {
                        block,
                        buf,
                        written,
                    };
                    return self.internal_poll_write_block(cx);
                }
                *me.state = AsyncFileBufState::Writing {
                    block,
                    buf,
                    written,
                };
                Poll::Pending
            }
            _ => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In an Invalid State",
            ))),
        }
    }
}

impl<F: AsyncRead + AsyncSeek + AsyncWrite> AsyncWrite for FileBufferAsync<F> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut written = 0;

        let block_size = self.buffer.block_size as usize;
        while written < buf.len() {
            let me = self.as_mut().project();
            match me.buffer.get_next_block_modify() {
                Ok((offset, data)) => {
                    let block = &mut data[offset..];
                    let b = &buf[written..];

                    let to_write = block.len().min(b.len());
                    (&mut block[..to_write]).copy_from_slice(&b[..to_write]);
                    written += to_write;

                    if data.len() < block_size {
                        let b = &buf[written..];
                        let to_extend = (block_size - data.len()).min(b.len());
                        data.extend_from_slice(&b[..to_extend]);
                        me.buffer.cursor += to_extend as u64;
                    }

                    me.buffer.cursor += to_write as u64;

                    continue;
                }
                Err((offset, block)) => {
                    let me = self.as_mut().project();
                    let buf = &buf[written..];

                    if offset == 0 && buf.len() > block_size {
                        // Overwrite the whole block without reading it
                        me.buffer.insert(block, Vec::from(&buf[..block_size]));
                        me.buffer.mark_modified(block);
                        written += block_size;
                        me.buffer.cursor += block_size as u64;
                        continue;
                    }

                    // Return the number of successful bytes written if any
                    // before making a call to the file
                    if written > 0 {
                        return Poll::Ready(Ok(written));
                    }

                    if self
                        .as_mut()
                        .internal_poll_read_block(cx, block)?
                        .is_pending()
                    {
                        return Poll::Pending;
                    }
                    continue;
                }
            }
        }

        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        if self.state.is_writing() {
            if self.as_mut().internal_poll_write_block(cx)?.is_pending() {
                return Poll::Pending;
            }
        }
        while let Some(block) = self.buffer.get_next_modified_block() {
            if self
                .as_mut()
                .internal_start_write_block(cx, block)?
                .is_pending()
            {
                return Poll::Pending;
            }
        }

        let me = self.as_mut().project();
        if me.file.poll_flush(cx)?.is_pending() {
            return Poll::Pending;
        }

        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        if self.state.is_closing() {
            return self.project().file.poll_shutdown(cx);
        }
        if self.as_mut().poll_flush(cx)?.is_pending() {
            return Poll::Pending;
        }
        let me = self.project();
        *me.state = AsyncFileBufState::Closing;
        me.file.poll_shutdown(cx)
    }
}

#[cfg(test)]
mod test {
    use rand::{rngs::StdRng, Rng as _, SeedableRng};
    use std::{
        io::{Cursor, Seek, Write},
        slice,
    };

    use super::*;
    const FILE: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nisl pretium fusce id velit ut tortor pretium viverra. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin. Gravida neque convallis a cras semper auctor neque vitae tempus. Cursus eget nunc scelerisque viverra mauris in aliquam. Viverra maecenas accumsan lacus vel facilisis volutpat est velit. Pulvinar mattis nunc sed blandit libero volutpat sed cras ornare. Massa eget egestas purus viverra accumsan in nisl nisi scelerisque. Ornare massa eget egestas purus viverra accumsan in nisl. Sed risus ultricies tristique nulla aliquet enim tortor. Laoreet suspendisse interdum consectetur libero id faucibus nisl tincidunt. Nisl tincidunt eget nullam non. Pretium quam vulputate dignissim suspendisse in est. Non enim praesent elementum facilisis. Nibh mauris cursus mattis molestie a. Iaculis nunc sed augue lacus viverra vitae. In mollis nunc sed id semper risus. Augue neque gravida in fermentum et sollicitudin ac. Pellentesque pulvinar pellentesque habitant morbi tristique senectus. Libero nunc consequat interdum varius sit.

Iaculis eu non diam phasellus vestibulum lorem sed risus ultricies. Vitae ultricies leo integer malesuada nunc. Enim lobortis scelerisque fermentum dui faucibus in ornare. Et netus et malesuada fames. Dignissim enim sit amet venenatis urna cursus. Volutpat maecenas volutpat blandit aliquam etiam erat velit scelerisque in. Viverra nibh cras pulvinar mattis nunc sed blandit libero. Condimentum id venenatis a condimentum. Blandit cursus risus at ultrices. Auctor eu augue ut lectus arcu. Felis imperdiet proin fermentum leo vel. Imperdiet dui accumsan sit amet nulla facilisi morbi tempus. Sed velit dignissim sodales ut eu sem integer vitae. Auctor urna nunc id cursus metus. Mattis pellentesque id nibh tortor id aliquet. Vitae auctor eu augue ut lectus arcu bibendum. Nisl condimentum id venenatis a condimentum vitae. Fusce id velit ut tortor pretium. Dignissim enim sit amet venenatis urna cursus eget. Sit amet mauris commodo quis.

Aliquam nulla facilisi cras fermentum odio eu feugiat pretium nibh. Tellus id interdum velit laoreet id donec ultrices tincidunt. Facilisis leo vel fringilla est ullamcorper eget. Orci phasellus egestas tellus rutrum tellus pellentesque. Enim nunc faucibus a pellentesque sit amet porttitor eget dolor. Cursus risus at ultrices mi tempus. Vitae auctor eu augue ut lectus arcu bibendum. Adipiscing elit duis tristique sollicitudin nibh sit amet commodo. Cursus mattis molestie a iaculis at erat pellentesque adipiscing. Suspendisse in est ante in nibh mauris. Scelerisque in dictum non consectetur a erat nam at lectus. Amet tellus cras adipiscing enim eu.

Sem nulla pharetra diam sit amet nisl suscipit adipiscing bibendum. Quam pellentesque nec nam aliquam sem et tortor consequat id. In nibh mauris cursus mattis molestie. Fermentum et sollicitudin ac orci phasellus egestas tellus. Volutpat maecenas volutpat blandit aliquam etiam erat velit scelerisque. Sollicitudin aliquam ultrices sagittis orci a scelerisque purus. Molestie nunc non blandit massa enim nec dui nunc. Ac ut consequat semper viverra nam libero. Quam elementum pulvinar etiam non quam. In hac habitasse platea dictumst vestibulum rhoncus est. Volutpat est velit egestas dui id ornare. Sed sed risus pretium quam vulputate dignissim suspendisse. Lorem sed risus ultricies tristique. Nibh sit amet commodo nulla facilisi nullam vehicula. Vel pretium lectus quam id leo in vitae turpis massa.

Nec ullamcorper sit amet risus nullam eget felis. Vestibulum mattis ullamcorper velit sed ullamcorper morbi. Interdum velit euismod in pellentesque massa placerat. Phasellus faucibus scelerisque eleifend donec pretium vulputate. Amet nisl suscipit adipiscing bibendum. Quam viverra orci sagittis eu volutpat odio facilisis mauris. Gravida dictum fusce ut placerat. Eget duis at tellus at urna condimentum mattis pellentesque. Est pellentesque elit ullamcorper dignissim cras. Iaculis nunc sed augue lacus viverra vitae congue eu consequat.";

    const SHORT: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nisl pretium fusce id velit ut tortor pretium viverra. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin. Gravida neque convallis a cras semper auctor neque vitae tempus. Cursus eget nunc scelerisque viverra mauris in aliquam. Viverra maecenas accumsan lacus vel facilisis volutpat est velit. Pulvinar mattis nunc sed blandit libero volutpat sed cras ornare. Massa eget egestas purus viverra accumsan in nisl nisi scelerisque. Ornare massa eget egestas purus viverra accumsan in nisl. Sed risus ultricies tristique nulla aliquet enim tortor. Laoreet suspendisse interdum consectetur libero id faucibus nisl tincidunt. Nisl tincidunt eget nullam non. Pretium quam vulputate dignissim suspendisse in est. Non enim praesent elementum facilisis. Nibh mauris cursus mattis molestie a. Iaculis nunc sed augue lacus viverra vitae. In mollis nunc sed id semper risus. Augue neque gravida in fermentum et sollicitudin ac. Pellentesque pulvinar pellentesque habitant morbi tristique senectus. Libero nunc consequat interdum varius sit.";

    #[test]
    fn test_read() {
        let mut cursor = Cursor::new(FILE);
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        let mut buf = [0u8; 100];
        for i in 0..(FILE.len() / 100) {
            let count = f.read(&mut buf).unwrap();
            assert_eq!(count, 100);
            assert_eq!(buf.as_slice(), &FILE[i * 100..(i + 1) * 100]);
        }

        let count = f.read(&mut buf).unwrap();
        assert_eq!(count, 12);
        assert_eq!(&buf[..count], &FILE[4100..4112]);
    }

    #[test]
    fn test_write() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        let count = f.write(SHORT).unwrap();
        assert_eq!(count, SHORT.len());
        f.flush().unwrap();

        cursor.rewind().unwrap();

        let mut buf = vec![0u8; SHORT.len()];
        let read = cursor.read(&mut buf).unwrap();
        assert_eq!(read, SHORT.len());
        assert_eq!(buf.as_slice(), SHORT);
    }

    #[test]
    fn test_replace() {
        let mut cursor = Cursor::new(Vec::from(FILE));
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        let written = f.write(b"Hello World").unwrap();
        assert_eq!(written, 11);
        f.flush().unwrap();

        cursor.rewind().unwrap();

        let mut buf = vec![0u8; 20];
        let read = cursor.read(&mut buf).unwrap();
        assert_eq!(read, 20);
        assert_eq!(buf.as_slice(), b"Hello World dolor si");
    }

    #[test]
    fn test_append() {
        let mut cursor = Cursor::new(Vec::from(SHORT));
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        let written = f.write(FILE).unwrap();
        assert_eq!(written, FILE.len());
        f.flush().unwrap();

        cursor.rewind().unwrap();

        let mut buf = vec![0u8; FILE.len()];
        let read = cursor.read(&mut buf).unwrap();
        assert_eq!(read, FILE.len());
        assert_eq!(buf.as_slice(), FILE);
    }

    #[test]
    fn test_replace_arbitrary() {
        let mut cursor = Cursor::new(Vec::from(FILE));
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        f.seek(SeekFrom::Start(9)).unwrap();
        let written = f.write(b"Hello World").unwrap();
        assert_eq!(written, 11);
        f.flush().unwrap();

        cursor.rewind().unwrap();

        let mut buf = vec![0u8; 30];
        let read = cursor.read(&mut buf).unwrap();
        assert_eq!(read, 30);
        assert_eq!(buf.as_slice(), b"Lorem ipsHello Worldt amet, co");
    }

    #[test]
    fn test_read_cache_limit() {
        let mut cursor = Cursor::new(FILE);
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .with_max_blocks(13)
            .build_sync(&mut cursor);

        let mut buf = [0u8; 100];
        for i in 0..(FILE.len() / 100) {
            let count = f.read(&mut buf).unwrap();
            assert_eq!(count, 100);
            assert_eq!(buf.as_slice(), &FILE[i * 100..(i + 1) * 100]);
            assert!(f.buffer.blocks.len() <= 13);
        }

        let count = f.read(&mut buf).unwrap();
        assert_eq!(count, 12);
        assert_eq!(&buf[..count], &FILE[4100..4112]);
    }

    #[test]
    fn test_read_after_end() {
        let mut cursor = Cursor::new(FILE);
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .build_sync(&mut cursor);

        let mut buf = [0u8; 100];
        f.seek(SeekFrom::End(100)).unwrap();
        let read = f.read(&mut buf).unwrap();
        assert_eq!(read, 0);
    }

    #[test]
    fn test_random_writes() {
        let mut file = Vec::from(FILE);
        let mut cursor = Cursor::new(&mut file);
        let mut f = FileBufferBuilder::new()
            .with_block_size(13)
            .with_max_blocks(13)
            .build_sync(&mut cursor);

        let mut rng = StdRng::from_seed(*b"Hiya World This is a random seed");
        // let mut rng = StdRng::from_entropy();

        for _ in 0..1000 {
            let i = rng.gen_range(0..FILE.len());
            let c = rng.gen_range(0u8..255);

            f.seek(SeekFrom::Start(i as u64)).unwrap();
            f.write(slice::from_ref(&c)).unwrap();
        }
        f.flush().unwrap();

        let mut buf = vec![0u8; FILE.len()];
        f.rewind().unwrap();
        f.read_exact(buf.as_mut_slice()).unwrap();

        assert_eq!(file, buf);
    }
}
