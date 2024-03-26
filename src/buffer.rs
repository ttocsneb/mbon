//! A wrapper for files.
//!
//! [FileBuffer] wraps another Reader/Writer and is able
//! to hold a large buffer of the file and allows for seeks without clearing the
//! buffer. The buffer has a limited capacity which can be set with
//! [FileBufferOptions::with_max_cache()]/[FileBufferOptions::with_max_blocks()].
//!
//! It does this by internally storing a series of blocks each of a
//! predetermined size ([FileBufferOptions::with_block_size()]). When the buffer
//! gets too big, then the least recently used blocks will be removed from the
//! cache.
//!
//! This wrapper is most useful for applications where the file is seeked often
//! and many reads/writes happen close together.
//!
//! In order to create a [FileBuffer], the
//! [FileBufferOptions] must be used.
use std::{
    collections::{BTreeSet, BinaryHeap, HashMap},
    mem,
};

#[cfg(feature = "async-tokio")]
mod async_buf;
#[cfg(feature = "sync")]
mod sync_buf;

#[cfg(feature = "sync")]
use std::io::{Read, Seek};

#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncSeek};

#[cfg(feature = "sync")]
pub use self::sync_buf::FileBuffer;

#[cfg(feature = "async-tokio")]
pub use self::async_buf::FileBufferAsync as FileBuffer;

struct Block {
    data: Vec<u8>,
    access: u64,
}

/// The internal buffer used by [FileBuffer].
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
    /// Remove the least recently used blocks that have not been marked as
    /// modified.
    ///
    /// This will reduce the blocks down to [Self::ideal_blocks].
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

    /// Purge the cache only if the cache is full.
    ///
    /// The cache is considered full when there are more than [Self::max_blocks].
    fn purge_if_full(&mut self) {
        if self.is_full() {
            self.purge_least_recently_used();
        }
    }

    /// Get next block the cursor is pointing to.
    ///
    /// If the block exists, then [Ok] will be returned with the offset within
    /// the block and the contents of the block.
    ///
    /// If the block doesn't exist, then [Err] will be returned with the offset
    /// within the block and the block id.
    fn get_next_block(&mut self) -> Result<(usize, &mut Vec<u8>), (usize, u64)> {
        let block = self.cursor / self.block_size;
        let offset = (self.cursor % self.block_size) as usize;
        match get_block!(mut self, block) {
            Some(data) => Ok((offset, data)),
            None => Err((offset, block)),
        }
    }

    /// Get next block the cursor is pointing to and mark it as modified.
    ///
    /// If the block exists, then [Ok] will be returned with the offset within
    /// the block and the contents of the block.
    ///
    /// If the block doesn't exist, then [Err] will be returned with the offset
    /// within the block and the block id.
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

    /// Get the next block id that has been marked as modified.
    fn get_next_modified_block(&self) -> Option<u64> {
        self.modified.first().copied()
    }

    /// reset all blocks to be unmodified and return all that were previously
    /// marked as modified
    #[allow(unused)]
    fn take_modified(&mut self) -> Vec<u64> {
        let mut modified: Vec<_> = mem::take(&mut self.modified).into_iter().collect();
        modified.sort_unstable();
        modified
    }

    /// Get the data from a block id.
    #[allow(unused)]
    fn get_block_mut(&mut self, block: u64) -> Option<&mut Vec<u8>> {
        get_block!(mut self, block)
    }

    /// Check if the cache is full.
    ///
    /// The cache is considered full when there are more than [Self::max_blocks].
    fn is_full(&self) -> bool {
        self.blocks.len() > self.max_blocks
    }

    /// Insert a new block into the buffer.
    ///
    /// The data is inserted into `block` id as is.
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

    /// Mark a block as modified
    #[inline]
    fn mark_modified(&mut self, block: u64) {
        self.modified.insert(block);
    }

    /// Mark a block as unmodified
    #[inline]
    fn mark_clean(&mut self, block: u64) {
        self.modified.remove(&block);
    }
}

/// Set options for a [FileBuffer].
///
/// There are three options that can be set.
///
/// * [Self::with_block_size()] Set the number of bytes in a block (default: 512)
/// * [Self::with_max_blocks()]/[Self::with_max_cache()] Set the max number of blocks in cache (default: 1GiB)
/// * [Self::with_ideal_blocks()]/[Self::with_ideal_blocks()] Set the number of blocks to reduce by (default: 1MiB)
///
#[derive(Default, Clone, PartialEq, Eq)]
pub struct FileBufferOptions {
    block_size: Option<u64>,
    max_blocks: Option<usize>,
    ideal_blocks: Option<usize>,
    max_cache: Option<u64>,
    ideal_cache: Option<u64>,
}

impl FileBufferOptions {
    /// Create a new [FileBufferOptions] object
    #[inline]
    pub fn new() -> Self {
        FileBufferOptions::default()
    }

    /// Set the number of bytes in a block.
    ///
    /// The default is 512 bytes.
    pub fn with_block_size(&mut self, block_size: u64) -> &mut Self {
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
    pub fn with_max_blocks(&mut self, max_blocks: usize) -> &mut Self {
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
    pub fn with_max_cache(&mut self, max_cache: u64) -> &mut Self {
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
    pub fn with_ideal_blocks(&mut self, ideal_blocks: usize) -> &mut Self {
        self.ideal_blocks = Some(ideal_blocks);
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
    pub fn with_ideal_cache(&mut self, ideal_cache: u64) -> &mut Self {
        self.ideal_cache = Some(ideal_cache);
        self.ideal_blocks = None;
        self
    }

    /// Build the Buffer
    fn internal_build(&self) -> Buffer {
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

    /// Build a [FileBuffer] with a given stream.
    ///
    /// The stream must be at least a [Read] + [Seek]
    #[cfg(feature = "sync")]
    pub fn build<F: Read + Seek>(&self, f: F) -> FileBuffer<F> {
        let buffer = self.internal_build();

        FileBuffer::new(buffer, f)
    }

    /// Build a [FileBuffer] with a given async stream.
    ///
    /// The stream must be at least a [AsyncRead] + [AsyncSeek]
    ///
    /// This function is only available with the feature `async-tokio` enabled.
    #[cfg(feature = "async-tokio")]
    pub fn build<F: AsyncRead + AsyncSeek>(&self, f: F) -> FileBuffer<F> {
        let buffer = self.internal_build();

        FileBuffer::new(buffer, f)
    }
}

#[cfg(test)]
mod test_suite {
    use std::{
        fs,
        io::Read,
        panic::{self, UnwindSafe},
        path::{Path, PathBuf},
    };

    use rand::{thread_rng, Rng as _};

    const FILES: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test");

    pub fn lorem_ipsom() -> PathBuf {
        Path::new(FILES).join("lorem_ipsom.txt")
    }

    pub fn lorem_ipsom_content() -> Vec<u8> {
        let mut buf = Vec::new();
        fs::File::open(lorem_ipsom())
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();
        buf
    }

    pub fn file_name(base: &str, id: &str, ext: &str) -> PathBuf {
        Path::new(FILES).join(format!("{base}_{id}.{ext}"))
    }

    pub fn copy_lorem_ipsom(id: &str) -> PathBuf {
        let foo = file_name("lorem_ipsom_copy", id, "txt");
        fs::copy(Path::new(FILES).join("lorem_ipsom.txt"), &foo).unwrap();
        foo
    }

    fn find_all_files(path: &Path) -> Vec<PathBuf> {
        let mut ents = Vec::new();
        for ent in fs::read_dir(path).unwrap() {
            if ent.is_err() {
                continue;
            }
            let ent = ent.unwrap();
            let meta = ent.metadata();
            if meta.is_err() {
                continue;
            }
            let meta = meta.unwrap();

            if meta.is_dir() {
                ents.extend(find_all_files(ent.path().as_ref()));
            } else {
                ents.push(ent.path());
            }
        }

        ents
    }

    pub fn run_test(test: impl FnOnce(&str) + UnwindSafe) {
        let next_byte = || {
            const CHOICES: &'static str =
                "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
            CHOICES
                .chars()
                .nth(thread_rng().gen_range(0..CHOICES.len()))
                .unwrap()
        };

        let mut buf = [' '; 16];
        for i in 0..buf.len() {
            buf[i] = next_byte();
        }
        let id = buf.into_iter().collect::<String>();

        println!("Setup");

        // let err = if _sync {
        let err = panic::catch_unwind(|| test(&id)).err();
        // } else {
        // test(id.clone()).catch_unwind().await.err()
        // };

        println!("Teardown");

        for file in find_all_files(FILES.as_ref()) {
            if let Some(name) = file.file_name() {
                if name.to_string_lossy().find(&id).is_some() {
                    println!("Removing {file:?}");
                    fs::remove_file(file).unwrap();
                }
            }
        }

        if let Some(err) = err {
            panic::panic_any(err)
        }
    }

    /// Call [run_test()] as a wrapper so that any created files will be removed
    /// afterwards, even if the test fails.
    ///
    /// ```
    /// use crate::file_test;
    ///
    /// file_test!(fn my_test() {
    ///     assert_eq!(1, 1);
    /// })
    ///
    /// file_test!(async fn my_test() {
    ///     assert_eq!(1, 1);
    /// })
    /// ```
    #[macro_export]
    macro_rules! file_test {
        (fn $test:ident($($arg:ident: $type:ty),*) $body:expr) => {
            #[test]
            fn $test() {
                run_test(|$($arg: $type),*| $body)
            }
        };
        (async fn $test:ident($($arg:ident: $type:ty),*) $body:expr) => {
            #[test]
            fn $test() {
                run_test(|$($arg: $type),*| {
                    tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async move {$body});
                })
            }
        };
    }
}
