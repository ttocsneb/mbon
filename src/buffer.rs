//! Contains [BufferedReadWrite], which is a wrapper for files.
//!
//! It currently can only be implemented synchronously which requires that
//! operations are executed in a spawn_blocking context. This isn't the worst,
//! but it would be nice to be able to utilize async io as it is natively
//! supported by tokio.
//!
//! I can't just write an AsyncRead/AsyncWrite wrapper since it requires a state
//! that would make the current implementation way too complex. If I were to
//! implement AsyncReadExt/AsyncWriteExt which have a nicer, I would also need
//! to implement the base traits. It's possible that I could just implement the
//! base trait and panic if the base trait is called, but I don't know how I
//! feel about that.
//!
//! I could make a custom trait that all asyncReadExt objects would implement,
//! but then users would need to import that custom trait whenever they are
//! using the engine which doesn't sound great either.
//!
//! Also, how would I deal with files that are provided that are sync only, such
//! as with a `vec<u8>`? When in async mode, I would have to have two
//! implementations available for whether F is async or not.

use std::{
    collections::{BinaryHeap, HashMap, HashSet},
    mem,
};

use std::io::{self, Read, Seek, SeekFrom, Write};

struct Block {
    data: Vec<u8>,
    access: u64,
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
pub struct BufferedReadWrite<F> {
    blocks: HashMap<u64, Block>,
    modified: HashSet<u64>,
    file: F,
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

/// Builder for [BufferedReadWrite].
pub struct BufferedReadWriteBuilder<F> {
    file: F,
    block_size: Option<u64>,
    max_blocks: Option<usize>,
    ideal_blocks: Option<usize>,
    max_cache: Option<u64>,
    ideal_cache: Option<u64>,
}

impl<F> BufferedReadWriteBuilder<F> {
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

    /// Create the BufferedReadWrite object
    pub fn build(self) -> BufferedReadWrite<F> {
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
        // .unwrap_or_else(|| (self.rec_cache.unwrap_or(1_000_000_000) / block_size) as usize);
        BufferedReadWrite {
            blocks: HashMap::new(),
            modified: HashSet::new(),
            file: self.file,
            cursor: 0,
            block_size,
            max_blocks,
            ideal_blocks,
            access_count: 0,
        }
    }
}

impl<F> From<F> for BufferedReadWrite<F>
where
    F: Seek,
{
    fn from(value: F) -> Self {
        Self::new(value).build()
    }
}

impl<F> BufferedReadWrite<F>
where
    F: Seek,
{
    /// Create a new BufferedReadWriteBuilder.
    ///
    /// ```no_run
    /// use mbon::buffer::BufferedReadWrite;
    /// use std::fs::File;
    ///
    /// let file = File::options()
    ///     .read(true)
    ///     .write(true)
    ///     .create(true)
    ///     .open("my_file.mbon").unwrap();
    /// let f = BufferedReadWrite::new(file).build();
    /// ```
    #[inline]
    pub fn new(file: F) -> BufferedReadWriteBuilder<F> {
        BufferedReadWriteBuilder {
            file,
            block_size: None,
            max_blocks: None,
            max_cache: None,
            ideal_cache: None,
            ideal_blocks: None,
        }
    }

    /// Purge the n_blocks least recently used blocks from the cache.
    ///
    /// This will ignore any blocks that have been modified.
    fn purge_least_recently_used(&mut self, n_blocks: usize) {
        println!(
            "Clearing {n_blocks} blocks to {}",
            self.blocks.len() - n_blocks
        );
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

    /// Clear the cache without flushing the file.
    ///
    /// This will preserve any cached blocks that have been modified.
    pub fn clear_cache_no_flush(&mut self) {
        let blocks = mem::take(&mut self.blocks);
        self.blocks = blocks
            .into_iter()
            .filter(|(k, _)| self.modified.contains(k))
            .collect();
    }
}

impl<F> BufferedReadWrite<F>
where
    F: Write + Seek,
{
    fn flush_blocks(&mut self) -> io::Result<()> {
        // I'm sorting here because I would assume that it is quicker for the file system to write
        // in order than it would be to write in a random order.
        let mut modified: Vec<_> = mem::take(&mut self.modified).into_iter().collect();
        modified.sort_unstable();

        let mut position = match modified.first() {
            Some(sect) => self.file.seek(SeekFrom::Start(sect * self.block_size))?,
            None => self.file.seek(SeekFrom::Current(0))?,
        };

        for sect in modified {
            let buf = match get_block!(mut self, sect) {
                Some(b) => b,
                None => continue,
            };
            let pos = sect * self.block_size;
            if position != pos {
                self.file.seek(SeekFrom::Start(pos))?;
                position = pos;
            }

            self.file.write_all(buf.as_slice())?;
            position += buf.len() as u64;
        }
        self.file.flush()?;

        if self.blocks.len() > self.max_blocks {
            self.purge_least_recently_used(self.blocks.len() - self.ideal_blocks);
        }

        Ok(())
    }

    /// Clear the cache
    ///
    /// If there are any modified changes, they will be written to disk before
    /// clearing the cache.
    pub fn clear_cache(&mut self) -> io::Result<()> {
        self.flush_blocks()?;
        self.blocks.clear();
        Ok(())
    }
}

impl<F> BufferedReadWrite<F>
where
    F: Read + Seek,
{
    fn load_blocks(&mut self, position: u64, len: u64) -> io::Result<()> {
        let end = position + len;
        let block = position / self.block_size;
        let end_block = (end + self.block_size - 1) / self.block_size;
        let num_blocks = end_block - block;

        let mut to_load = Vec::new();

        for sect in block..block + num_blocks {
            if !self.blocks.contains_key(&sect) {
                to_load.push(sect);
            }
        }

        let mut position = self.file.seek(SeekFrom::Current(0))?;

        for sect in to_load {
            let pos = sect * self.block_size;
            if position != pos {
                self.file.seek(SeekFrom::Start(pos))?;
                position = pos;
            }
            let mut buf = vec![0u8; self.block_size as usize];
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

            self.blocks.insert(
                sect,
                Block {
                    data: buf,
                    access: self.access_count,
                },
            );
            self.access_count += 1;
            if eof {
                break;
            }
        }

        Ok(())
    }
}

impl<F> Read for BufferedReadWrite<F>
where
    F: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.load_blocks(self.cursor, buf.len() as u64)?;
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
            self.purge_least_recently_used(self.blocks.len() - self.ideal_blocks);
        }

        Ok(offset)
    }
}

impl<F> Write for BufferedReadWrite<F>
where
    F: Read + Write + Seek,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.load_blocks(self.cursor, buf.len() as u64)?;
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

        if self.blocks.len() > self.max_blocks {
            self.flush_blocks()?;
        }

        Ok(offset)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_blocks()
    }
}

impl<F> Seek for BufferedReadWrite<F>
where
    F: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => self.cursor = p,
            SeekFrom::End(_) => {
                self.cursor = self.file.seek(pos)?;
            }
            SeekFrom::Current(o) => {
                self.cursor = self.cursor.checked_add_signed(o).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot seek to a negative position",
                    )
                })?
            }
        }
        Ok(self.cursor)
    }
}

#[cfg(test)]
mod test {
    use rand::{rngs::StdRng, Rng as _, SeedableRng};
    use std::{
        io::{Cursor, Read, Seek, Write},
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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

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
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .with_max_blocks(13)
            .build();

        let mut buf = [0u8; 100];
        for i in 0..(FILE.len() / 100) {
            let count = f.read(&mut buf).unwrap();
            assert_eq!(count, 100);
            assert_eq!(buf.as_slice(), &FILE[i * 100..(i + 1) * 100]);
            assert!(f.blocks.len() <= 13);
        }

        let count = f.read(&mut buf).unwrap();
        assert_eq!(count, 12);
        assert_eq!(&buf[..count], &FILE[4100..4112]);
    }

    #[test]
    fn test_read_after_end() {
        let mut cursor = Cursor::new(FILE);
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .build();

        let mut buf = [0u8; 100];
        f.seek(SeekFrom::End(100)).unwrap();
        let read = f.read(&mut buf).unwrap();
        assert_eq!(read, 0);
    }

    #[test]
    fn test_random_writes() {
        let mut file = Vec::from(FILE);
        let mut cursor = Cursor::new(&mut file);
        let mut f = BufferedReadWrite::new(&mut cursor)
            .with_block_size(13)
            .with_max_blocks(13)
            .build();

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
