use super::Buffer;

use std::io::{self, SeekFrom};
use std::{
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use enum_as_inner::EnumAsInner;
use pin_project::pin_project;
use strum::AsRefStr;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

#[derive(EnumAsInner, AsRefStr)]
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

/// [FileBufferAsync] wraps another AsyncReader/AsyncWriter and is able
/// to hold a large buffer of the file and allows for seeks without clearing the
/// buffer. The buffer has a limited capacity which can be set with
/// [super::FileBufferOptions::with_max_cache()]/[super::FileBufferOptions::with_max_blocks()].
///
/// It does this by internally storing a series of blocks each of a
/// predetermined size ([super::FileBufferOptions::with_block_size()]). When the
/// buffer gets too big, then the least recently used blocks will be removed
/// from the cache.
///
/// This wrapper is most useful for applications where the file is seeked often
/// and many reads/writes happen close together.
///
/// In order to create a [FileBufferAsync], the [super::FileBufferOptions] must
/// be used.
///
/// [FileBufferAsync] is only available when the feature `async-tokio` is
/// enabled.
///
/// ```no_run
/// # async {
/// use mbon::buffer::FileBufferOptions;
/// use tokio::fs::File;
///
/// let file = File::options()
///     .read(true)
///     .write(true)
///     .open("my_file.mbon").await.unwrap();
///
/// let fb = FileBufferOptions::new()
///     .with_block_size(4096)
///     .with_max_cache(1_000_000)
///     .build(file);
/// # };
/// ```
#[pin_project]
pub struct FileBufferAsync<F> {
    buffer: Buffer,
    #[pin]
    file: F,
    cursor: Option<u64>,
    state: AsyncFileBufState,
}
impl<F> FileBufferAsync<F> {
    pub(super) fn new(buffer: Buffer, file: F) -> Self {
        Self {
            buffer,
            file,
            cursor: None,
            state: AsyncFileBufState::default(),
        }
    }
}

impl<F: AsyncRead + AsyncSeek> FileBufferAsync<F> {
    fn internal_poll_read_block(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        block: u64,
    ) -> Poll<io::Result<bool>> {
        let state = mem::take(self.as_mut().project().state);
        let block_size = self.buffer.block_size;
        match state {
            AsyncFileBufState::Normal => {
                println!("Seeking to block {block}");
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
                println!("Reading block {block}");

                let mut b = ReadBuf::new(buf.as_mut_slice());
                b.set_filled(read);
                if me.file.poll_read(cx, &mut b)?.is_pending() {
                    *me.state = AsyncFileBufState::Reading { block, buf, read };
                    return Poll::Pending;
                }
                let filled = b.filled().len();
                let read_this_time = filled - read;
                let read = filled;
                *me.cursor = Some(block * me.buffer.block_size + read as u64);
                if b.remaining() == 0 {
                    me.buffer.insert(block, buf);
                    return Poll::Ready(Ok(true));
                }
                if read_this_time == 0 {
                    if read == 0 {
                        return Poll::Ready(Ok(false));
                    }
                    for i in (read..block_size as usize).rev() {
                        buf.remove(i);
                    }
                    me.buffer.insert(block, buf);
                    return Poll::Ready(Ok(true));
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
        let mut read = 0;
        println!("Reading into buf");
        while buf.remaining() > 0 {
            let me = self.as_mut().project();
            match me.buffer.get_next_block() {
                Ok((offset, block)) => {
                    println!("Next block is available");

                    let block = &block[offset..];
                    let to_read = block.len().min(buf.remaining());
                    if to_read == 0 {
                        me.buffer.purge_if_full();
                        println!("End of file");
                        return Poll::Ready(Ok(()));
                    }
                    read += to_read;
                    buf.put_slice(&block[..to_read]);
                    me.buffer.cursor += to_read as u64;
                    continue;
                }
                Err((_offset, block)) => {
                    if read > 0 {
                        println!("! Read Partial !");
                        return Poll::Ready(Ok(()));
                    }
                    println!("Need to read block {block}");
                    let poll = self.as_mut().internal_poll_read_block(cx, block)?;
                    if let Poll::Ready(exists) = poll {
                        if !exists {
                            let me = self.as_mut().project();
                            me.buffer.purge_if_full();
                            println!("Doesn't exist");
                            return Poll::Ready(Ok(()));
                        }
                    } else {
                        println!("Pending...");
                        return Poll::Pending;
                    }
                }
            }
        }

        self.project().buffer.purge_if_full();
        println!("! All Done !");
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
            println!("Starting seek to {position:?}");
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
            AsyncFileBufState::Normal => Poll::Ready(Ok(match self.cursor {
                Some(val) => val,
                None => 0,
            })),
            AsyncFileBufState::StartSeek(seek) => match seek {
                SeekFrom::Start(pos) => {
                    let me = self.as_mut().project();
                    if let Some(actual) = me.cursor {
                        println!("At {actual}, want to go to {pos}");
                        if *actual == pos {
                            println!("Already at {pos}, no action needed");
                            return Poll::Ready(Ok(*actual));
                        }
                    }
                    if me.file.poll_complete(cx)?.is_pending() {
                        *me.state = AsyncFileBufState::StartSeek(seek);
                        return Poll::Pending;
                    }
                    println!("!! Starting file seek");
                    let me = self.as_mut().project();
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
                println!("Are we there yet?");
                let poll = me.file.poll_complete(cx)?;
                if let Poll::Ready(pos) = poll {
                    *me.cursor = Some(pos);
                    return Poll::Ready(Ok(pos));
                }
                *me.state = AsyncFileBufState::Seeking;
                Poll::Pending
            }
            AsyncFileBufState::Reading { block, buf, read } => {
                let me = self.as_mut().project();
                *me.state = AsyncFileBufState::Reading { block, buf, read };
                Poll::Ready(Ok(match self.cursor {
                    Some(val) => val,
                    None => 0,
                }))
            }
            AsyncFileBufState::Writing {
                block,
                buf,
                written,
            } => {
                let me = self.as_mut().project();
                *me.state = AsyncFileBufState::Writing {
                    block,
                    buf,
                    written,
                };
                Poll::Ready(Ok(match self.cursor {
                    Some(val) => val,
                    None => 0,
                }))
            }
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
    fn internal_poll_write_block(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        block: u64,
    ) -> Poll<io::Result<()>> {
        let state = mem::take(self.as_mut().project().state);
        let block_size = self.buffer.block_size;

        match state {
            AsyncFileBufState::Normal => {
                println!("Going to write block {block}");

                if self
                    .as_mut()
                    .internal_cursor_try_seek(cx, SeekFrom::Start(block * block_size))?
                    .is_pending()
                {
                    println!("! Pending Seek !");
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
                self.internal_poll_write_block(cx, block)
            }
            AsyncFileBufState::Reading { .. } => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "In a Reading State",
            ))),
            AsyncFileBufState::Writing {
                block,
                buf,
                written,
            } => {
                let me = self.as_mut().project();
                debug_assert_eq!(*me.cursor, Some(block * block_size + written as u64));

                println!("Writing to block {block}");
                let poll = me.file.poll_write(cx, &buf[written..])?;
                if let Poll::Ready(w) = poll {
                    println!("Wrote {w} bytes");
                    let written = written + w;
                    *me.cursor = Some(block * block_size + written as u64);
                    if written == buf.len() {
                        me.buffer.mark_clean(block);
                        println!("! All Done !");
                        return Poll::Ready(Ok(()));
                    }

                    *me.state = AsyncFileBufState::Writing {
                        block,
                        buf,
                        written,
                    };
                    return self.internal_poll_write_block(cx, block);
                }
                *me.state = AsyncFileBufState::Writing {
                    block,
                    buf,
                    written,
                };
                println!("! Pending Write !");
                Poll::Pending
            }
            AsyncFileBufState::Closing => {
                let me = self.as_mut().project();
                *me.state = AsyncFileBufState::Closing;
                Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "Closed")))
            }
            state => {
                let me = self.as_mut().project();
                *me.state = state;

                let poll = self.as_mut().internal_cursor_poll_complete(cx)?;
                if poll.is_ready() {
                    return self.internal_poll_write_block(cx, block);
                }
                println!("! Pending Seek !");
                Poll::Pending
            }
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
                        me.buffer.purge_if_full();
                        return Poll::Ready(Ok(written));
                    }
                    let poll = self.as_mut().internal_poll_read_block(cx, block)?;

                    if let Poll::Ready(exists) = poll {
                        if !exists {
                            // Create a new block
                            let me = self.as_mut().project();
                            let to_write = block_size.min(buf.len());
                            me.buffer.insert(block, Vec::from(&buf[..to_write]));
                            me.buffer.mark_modified(block);
                            written += to_write;
                            me.buffer.cursor += to_write as u64;
                            continue;
                        }
                    } else {
                        return Poll::Pending;
                    }
                    continue;
                }
            }
        }

        self.project().buffer.purge_if_full();
        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        while let Some(block) = self.buffer.get_next_modified_block() {
            if self
                .as_mut()
                .internal_poll_write_block(cx, block)?
                .is_pending()
            {
                return Poll::Pending;
            }
        }

        let me = self.as_mut().project();
        if me.file.poll_flush(cx)?.is_pending() {
            return Poll::Pending;
        }

        me.buffer.purge_if_full();

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
    use super::super::test_suite::*;
    use crate::file_test;

    use super::super::FileBufferOptions;
    use super::*;

    use rand::{rngs::StdRng, Rng as _, SeedableRng};
    use std::slice;
    use tokio::{
        fs::File,
        io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    };

    const SHORT: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nisl pretium fusce id velit ut tortor pretium viverra. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin. Gravida neque convallis a cras semper auctor neque vitae tempus. Cursus eget nunc scelerisque viverra mauris in aliquam. Viverra maecenas accumsan lacus vel facilisis volutpat est velit. Pulvinar mattis nunc sed blandit libero volutpat sed cras ornare. Massa eget egestas purus viverra accumsan in nisl nisi scelerisque. Ornare massa eget egestas purus viverra accumsan in nisl. Sed risus ultricies tristique nulla aliquet enim tortor. Laoreet suspendisse interdum consectetur libero id faucibus nisl tincidunt. Nisl tincidunt eget nullam non. Pretium quam vulputate dignissim suspendisse in est. Non enim praesent elementum facilisis. Nibh mauris cursus mattis molestie a. Iaculis nunc sed augue lacus viverra vitae. In mollis nunc sed id semper risus. Augue neque gravida in fermentum et sollicitudin ac. Pellentesque pulvinar pellentesque habitant morbi tristique senectus. Libero nunc consequat interdum varius sit.";

    file_test!(
        async fn test_read_async(_id: &str) {
            let lic = lorem_ipsom_content();

            let file = lorem_ipsom();
            let file = File::open(file).await.unwrap();

            let mut f = FileBufferOptions::new().with_block_size(13).build(file);

            let mut buf = [0u8; 100];
            for i in 0..(lic.len() / 100) {
                f.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf.as_slice(), &lic[i * 100..(i + 1) * 100]);
            }

            let mut buf = Vec::new();
            f.read_to_end(&mut buf).await.unwrap();
            assert_eq!(&buf, &lic[(lic.len() / 100) * 100..]);
        }
    );

    file_test!(
        async fn test_write_async(id: &str) {
            let file = file_name("lorem_ipsom_write", id, "txt");
            let mut file = File::options()
                .create(true)
                .read(true)
                .write(true)
                .open(file)
                .await
                .unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.write_all(SHORT).await.unwrap();
            f.flush().await.unwrap();

            AsyncSeekExt::rewind(&mut file).await.unwrap();

            let mut buf = vec![0u8; SHORT.len()];
            let read = AsyncReadExt::read(&mut file, &mut buf).await.unwrap();
            assert_eq!(read, SHORT.len());
            assert_eq!(buf.as_slice(), SHORT);
        }
    );

    file_test!(
        async fn test_replace_async(id: &str) {
            let file = copy_lorem_ipsom(id);
            let mut file = File::options()
                .read(true)
                .write(true)
                .open(file)
                .await
                .unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.write_all(b"Hello World").await.unwrap();
            f.flush().await.unwrap();

            AsyncSeekExt::rewind(&mut file).await.unwrap();

            let mut buf = vec![0u8; 20];
            let read = AsyncReadExt::read(&mut file, &mut buf).await.unwrap();
            assert_eq!(read, 20);
            assert_eq!(buf.as_slice(), b"Hello World dolor si");
        }
    );

    file_test!(
        async fn test_append_async(id: &str) {
            let lic = lorem_ipsom_content();

            let file = file_name("lorem_ipsom_append", id, "txt");
            let mut file = File::options()
                .create(true)
                .read(true)
                .write(true)
                .open(file)
                .await
                .unwrap();
            file.write_all(SHORT).await.unwrap();
            file.flush().await.unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.write_all(lic.as_slice()).await.unwrap();
            f.flush().await.unwrap();

            AsyncSeekExt::rewind(&mut file).await.unwrap();

            let mut buf = vec![0u8; lic.len()];
            let read = AsyncReadExt::read(&mut file, &mut buf).await.unwrap();
            assert_eq!(read, lic.len());
            assert_eq!(buf.as_slice(), lic);
        }
    );

    file_test!(
        async fn test_replace_arbitrary_async(id: &str) {
            let file = copy_lorem_ipsom(id);
            let mut file = File::options()
                .read(true)
                .write(true)
                .open(file)
                .await
                .unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.seek(SeekFrom::Start(9)).await.unwrap();
            f.write_all(b"Hello World").await.unwrap();
            f.flush().await.unwrap();

            AsyncSeekExt::rewind(&mut file).await.unwrap();

            let mut buf = vec![0u8; 30];
            let read = AsyncReadExt::read(&mut file, &mut buf).await.unwrap();
            assert_eq!(read, 30);
            assert_eq!(buf.as_slice(), b"Lorem ipsHello Worldt amet, co");
        }
    );

    file_test!(
        async fn test_read_cache_limit_async(_id: &str) {
            let lic = lorem_ipsom_content();

            let file = lorem_ipsom();
            let mut file = File::open(file).await.unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .with_max_blocks(13)
                .build(&mut file);

            let mut buf = [0u8; 100];
            for i in 0..(lic.len() / 100) {
                f.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf.as_slice(), &lic[i * 100..(i + 1) * 100]);
                assert!(f.buffer.blocks.len() <= 13);
            }

            let mut buf = Vec::new();
            f.read_to_end(&mut buf).await.unwrap();
            assert_eq!(&buf, &lic[(lic.len() / 100) * 100..]);
        }
    );

    file_test!(
        async fn test_read_after_end_async(_id: &str) {
            let file = lorem_ipsom();
            let mut file = File::open(file).await.unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            let mut buf = [0u8; 100];
            f.seek(SeekFrom::End(100)).await.unwrap();
            let read = f.read(&mut buf).await.unwrap();
            assert_eq!(read, 0);
        }
    );

    file_test!(
        async fn test_random_writes_async(id: &str) {
            let lic = lorem_ipsom_content();

            let file = copy_lorem_ipsom(id);
            let mut file = File::options()
                .read(true)
                .write(true)
                .open(file)
                .await
                .unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .with_max_blocks(13)
                .build(&mut file);

            let mut rng = StdRng::from_seed(*b"Hiya World This is a random seed");
            // let mut rng = StdRng::from_entropy();

            for _ in 0..1000 {
                let i = rng.gen_range(0..lic.len());
                let c = rng.gen_range(0u8..255);

                f.seek(SeekFrom::Start(i as u64)).await.unwrap();
                f.write_all(slice::from_ref(&c)).await.unwrap();
            }
            f.flush().await.unwrap();

            let mut buf = vec![0u8; lic.len()];
            f.rewind().await.unwrap();
            f.read_exact(buf.as_mut_slice()).await.unwrap();

            let mut expected = Vec::new();
            file.rewind().await.unwrap();
            file.read_to_end(&mut expected).await.unwrap();

            assert_eq!(expected, buf);
        }
    );
}
