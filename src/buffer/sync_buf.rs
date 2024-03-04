use std::io::{self, Read, Seek, SeekFrom, Write};

use super::Buffer;

/// [FileBuffer] wraps another Reader/Writer and is able
/// to hold a large buffer of the file and allows for seeks without clearing the
/// buffer. The buffer has a limited capacity which can be set with
/// [super::FileBufferOptions::with_max_cache()]/[super::FileBufferOptions::with_max_blocks()].
///
/// It does this by internally storing a series of blocks each of a
/// predetermined size ([super::FileBufferOptions::with_block_size()]). When the buffer
/// gets too big, then the least recently used blocks will be removed from the
/// cache.
///
/// This wrapper is most useful for applications where the file is seeked often
/// and many reads/writes happen close together.
///
/// In order to create a [FileBuffer], the [super::FileBufferOptions] must be used.
///
/// ```no_run
/// use mbon::buffer::FileBufferOptions;
/// use std::fs::File;
///
/// let file = File::options()
///     .read(true)
///     .write(true)
///     .open("my_file.mbon").unwrap();
///
/// let fb = FileBufferOptions::new()
///     .with_block_size(4096)
///     .with_max_cache(1_000_000)
///     .build(file);
/// ```
pub struct FileBuffer<F> {
    buffer: Buffer,
    file: F,
    cursor: Option<u64>,
}

impl<F> FileBuffer<F> {
    pub(super) fn new(buffer: Buffer, file: F) -> Self {
        Self {
            buffer,
            file,
            cursor: None,
        }
    }
}

impl<F: Seek> FileBuffer<F> {
    fn internal_cursor_seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match position {
            SeekFrom::Start(position) => {
                if let Some(actual) = &self.cursor {
                    if *actual == position {
                        return Ok(*actual);
                    }
                }
                let position = self.file.seek(SeekFrom::Start(position))?;
                self.cursor = Some(position);
                Ok(position)
            }
            seek => {
                let position = self.file.seek(seek)?;
                self.cursor = Some(position);
                Ok(position)
            }
        }
    }
}

impl<F: Seek> Seek for FileBuffer<F> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(pos) => {
                self.buffer.cursor = pos;
                Ok(self.buffer.cursor)
            }
            SeekFrom::Current(offset) => {
                self.buffer.cursor = match self.buffer.cursor.checked_add_signed(offset) {
                    Some(v) => v,
                    None => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Cannot seek to a negative position",
                        ))
                    }
                };
                Ok(self.buffer.cursor)
            }
            seek => {
                self.buffer.cursor = self.internal_cursor_seek(seek)?;
                Ok(self.buffer.cursor)
            }
        }
    }
}

impl<F: Read + Seek> FileBuffer<F> {
    fn internal_read_block(&mut self, block: u64) -> io::Result<bool> {
        let block_size = self.buffer.block_size;
        self.internal_cursor_seek(SeekFrom::Start(block * block_size))?;

        let mut buf = vec![0u8; block_size as usize];
        let mut read = 0;

        loop {
            debug_assert_eq!(self.cursor, Some(block * block_size + read as u64));

            let read_buf = &mut buf[read..];
            let just_read = self.file.read(read_buf)?;
            read += just_read;
            self.cursor = Some(block * block_size + read as u64);

            if read == block_size as usize {
                self.buffer.insert(block, buf);
                return Ok(true);
            }
            if just_read == 0 {
                if read == 0 {
                    return Ok(false);
                }
                for i in (read..block_size as usize).rev() {
                    buf.remove(i);
                }
                self.buffer.insert(block, buf);
                return Ok(true);
            }
        }
    }
}

impl<F: Read + Seek> Read for FileBuffer<F> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0;

        while read < buf.len() {
            match self.buffer.get_next_block() {
                Ok((offset, data)) => {
                    let data = &data[offset..];
                    let read_buf = &mut buf[read..];
                    let to_read = data.len().min(read_buf.len());
                    if to_read == 0 {
                        self.buffer.purge_if_full();
                        return Ok(read);
                    }
                    let data = &data[..to_read];
                    let read_buf = &mut read_buf[..to_read];
                    read_buf.copy_from_slice(&data);
                    read += to_read;
                    self.buffer.cursor += to_read as u64;
                    continue;
                }
                Err((_offset, block)) => {
                    if read > 0 {
                        self.buffer.purge_if_full();
                        return Ok(read);
                    }
                    let exists = self.internal_read_block(block)?;
                    if !exists {
                        self.buffer.purge_if_full();
                        return Ok(read);
                    }
                    continue;
                }
            }
        }

        self.buffer.purge_if_full();
        Ok(read)
    }
}

impl<F: Write + Seek> FileBuffer<F> {
    fn internal_write_block(&mut self, block: u64) -> io::Result<()> {
        let block_size = self.buffer.block_size;
        self.internal_cursor_seek(SeekFrom::Start(block * block_size))?;

        let buf = &self
            .buffer
            .blocks
            .get(&block)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Block does not exist"))?
            .data;
        let mut written = 0;

        loop {
            debug_assert_eq!(self.cursor, Some(block * block_size + written as u64));

            let write_buf = &buf[written..];
            let just_wrote = self.file.write(write_buf)?;
            written += just_wrote;
            self.cursor = Some(block * block_size + written as u64);
            if written == buf.len() {
                self.buffer.mark_clean(block);
                return Ok(());
            }
        }
    }
}

impl<F: Write + Read + Seek> Write for FileBuffer<F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        let block_size = self.buffer.block_size as usize;

        while written < buf.len() {
            match self.buffer.get_next_block_modify() {
                Ok((offset, data)) => {
                    let data_buf = &mut data[offset..];
                    let b = &buf[written..];

                    let to_write = data_buf.len().min(b.len());
                    (&mut data_buf[..to_write]).copy_from_slice(&b[..to_write]);
                    written += to_write;

                    if data.len() < block_size {
                        let b = &buf[written..];
                        let to_extend = (block_size - data.len()).min(b.len());
                        data.extend_from_slice(&b[..to_extend]);
                        self.buffer.cursor += to_extend as u64;
                    }

                    self.buffer.cursor += to_write as u64;

                    continue;
                }
                Err((offset, block)) => {
                    let buf = &buf[written..];

                    if offset == 0 && buf.len() > block_size {
                        // Overwrite the whole block without reading it
                        self.buffer.insert(block, Vec::from(&buf[..block_size]));
                        self.buffer.mark_modified(block);
                        written += block_size;
                        self.buffer.cursor += block_size as u64;
                        continue;
                    }

                    // Return the number of successful bytes written if any
                    // before making a call to the file
                    if written > 0 {
                        self.buffer.purge_if_full();
                        return Ok(written);
                    }

                    let exists = self.internal_read_block(block)?;
                    if !exists {
                        // Create a new block
                        let to_write = (block_size - offset).min(buf.len());
                        let mut data = vec![0u8; to_write + offset];

                        let data_buf = &mut data[offset..];
                        data_buf.copy_from_slice(&buf[..to_write]);

                        self.buffer.insert(block, data);
                        self.buffer.mark_modified(block);
                        written += to_write;
                        self.buffer.cursor += to_write as u64;
                        continue;
                    } else {
                    }

                    continue;
                }
            }
        }

        self.buffer.purge_if_full();
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        while let Some(block) = self.buffer.get_next_modified_block() {
            self.internal_write_block(block)?;
        }
        self.buffer.purge_if_full();
        self.file.flush()
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

#[cfg(test)]
mod test {
    use super::super::test_suite::*;
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use std::{
        fs::File,
        io::{Seek, Write},
        slice,
    };

    use crate::{buffer::FileBufferOptions, file_test};

    use super::*;

    const SHORT: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Nisl pretium fusce id velit ut tortor pretium viverra. Tincidunt nunc pulvinar sapien et ligula ullamcorper malesuada proin. Gravida neque convallis a cras semper auctor neque vitae tempus. Cursus eget nunc scelerisque viverra mauris in aliquam. Viverra maecenas accumsan lacus vel facilisis volutpat est velit. Pulvinar mattis nunc sed blandit libero volutpat sed cras ornare. Massa eget egestas purus viverra accumsan in nisl nisi scelerisque. Ornare massa eget egestas purus viverra accumsan in nisl. Sed risus ultricies tristique nulla aliquet enim tortor. Laoreet suspendisse interdum consectetur libero id faucibus nisl tincidunt. Nisl tincidunt eget nullam non. Pretium quam vulputate dignissim suspendisse in est. Non enim praesent elementum facilisis. Nibh mauris cursus mattis molestie a. Iaculis nunc sed augue lacus viverra vitae. In mollis nunc sed id semper risus. Augue neque gravida in fermentum et sollicitudin ac. Pellentesque pulvinar pellentesque habitant morbi tristique senectus. Libero nunc consequat interdum varius sit.";

    file_test!(
        fn test_read(_id: &str) {
            let lic = lorem_ipsom_content();

            let file = lorem_ipsom();
            let file = File::open(file).unwrap();
            let mut f = FileBufferOptions::new().with_block_size(13).build(file);

            let mut buf = [0u8; 100];
            for i in 0..(lic.len() / 100) {
                f.read_exact(&mut buf).unwrap();
                assert_eq!(buf.as_slice(), &lic[i * 100..(i + 1) * 100]);
            }

            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            assert_eq!(&buf, &lic[(lic.len() / 100) * 100..]);
        }
    );

    file_test!(
        fn test_write(id: &str) {
            let file = file_name("lorem_ipsom_write", id, "txt");
            let mut file = File::options()
                .create(true)
                .read(true)
                .write(true)
                .open(file)
                .unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.write_all(SHORT).unwrap();
            f.flush().unwrap();

            Seek::rewind(&mut file).unwrap();

            let mut buf = vec![0u8; SHORT.len()];
            let read = Read::read(&mut file, &mut buf).unwrap();
            assert_eq!(read, SHORT.len());
            assert_eq!(buf.as_slice(), SHORT);
        }
    );

    file_test!(
        fn test_replace(id: &str) {
            let file = copy_lorem_ipsom(id);
            let mut file = File::options().read(true).write(true).open(file).unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            let written = f.write(b"Hello World").unwrap();
            assert_eq!(written, 11);
            f.flush().unwrap();

            Seek::rewind(&mut file).unwrap();

            let mut buf = vec![0u8; 20];
            let read = Read::read(&mut file, &mut buf).unwrap();
            assert_eq!(read, 20);
            assert_eq!(buf.as_slice(), b"Hello World dolor si");
        }
    );

    file_test!(
        fn test_append(id: &str) {
            let lic = lorem_ipsom_content();

            let file = file_name("lorem_ipsom_append", id, "txt");
            let mut file = File::options()
                .create(true)
                .read(true)
                .write(true)
                .open(file)
                .unwrap();
            file.write_all(SHORT).unwrap();
            file.flush().unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.write_all(lic.as_slice()).unwrap();
            f.flush().unwrap();

            Seek::rewind(&mut file).unwrap();

            let mut buf = vec![0u8; lic.len()];
            let read = Read::read(&mut file, &mut buf).unwrap();
            assert_eq!(read, lic.len());
            assert_eq!(buf.as_slice(), lic);
        }
    );

    file_test!(
        fn test_replace_arbitrary(id: &str) {
            let file = copy_lorem_ipsom(id);
            let mut file = File::options().read(true).write(true).open(file).unwrap();

            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            f.seek(SeekFrom::Start(9)).unwrap();
            f.write_all(b"Hello World").unwrap();
            f.flush().unwrap();

            Seek::rewind(&mut file).unwrap();

            let mut buf = vec![0u8; 30];
            let read = Read::read(&mut file, &mut buf).unwrap();
            assert_eq!(read, 30);
            assert_eq!(buf.as_slice(), b"Lorem ipsHello Worldt amet, co");
        }
    );

    file_test!(
        fn test_read_cache_limit(_id: &str) {
            let lic = lorem_ipsom_content();

            let file = lorem_ipsom();
            let mut file = File::open(file).unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .with_max_blocks(13)
                .build(&mut file);

            let mut buf = [0u8; 100];
            for i in 0..(lic.len() / 100) {
                f.read_exact(&mut buf).unwrap();
                assert_eq!(buf.as_slice(), &lic[i * 100..(i + 1) * 100]);
                assert!(f.buffer.blocks.len() <= 13);
            }

            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            assert_eq!(&buf, &lic[(lic.len() / 100) * 100..]);
        }
    );

    file_test!(
        fn test_read_after_end(_id: &str) {
            let file = lorem_ipsom();
            let mut file = File::open(file).unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .build(&mut file);

            let mut buf = [0u8; 100];
            f.seek(SeekFrom::End(100)).unwrap();
            let read = f.read(&mut buf).unwrap();
            assert_eq!(read, 0);
        }
    );

    file_test!(
        fn test_random_writes(id: &str) {
            let lic = lorem_ipsom_content();

            let file = copy_lorem_ipsom(id);
            let mut file = File::options().read(true).write(true).open(file).unwrap();
            let mut f = FileBufferOptions::new()
                .with_block_size(13)
                .with_max_blocks(13)
                .build(&mut file);

            let mut rng = StdRng::from_seed(*b"Hiya World This is a random seed");
            // let mut rng = StdRng::from_entropy();

            for _ in 0..1000 {
                let i = rng.gen_range(0..lic.len());
                let c = rng.gen_range(0u8..255);

                f.seek(SeekFrom::Start(i as u64)).unwrap();
                f.write_all(slice::from_ref(&c)).unwrap();
            }
            f.flush().unwrap();

            let mut buf = vec![0u8; lic.len()];
            f.rewind().unwrap();
            f.read_exact(buf.as_mut_slice()).unwrap();

            let mut expected = Vec::new();
            file.rewind().unwrap();
            file.read_to_end(&mut expected).unwrap();

            assert_eq!(expected, buf);
        }
    );
}
