//! # Async Wrappers for Dumper and Parser
//!
//! Async wrappers are provided for [Dumper]: [AsyncDumper] and [Parser].
//!
//! [Dumper]: crate::dumper::Dumper
//! [Parser]: crate::parser::Parser

use std::mem;

use crate::data::Value;
use crate::dumper::Dumper;
use crate::error::Result;
use crate::object::ObjectDump;

use futures::AsyncWriteExt;
use serde::Serialize;

/// A wrapper for [Dumper](crate::dumper::Dumper).
///
/// AsyncDumper writes to a buffer that can be sent to the writer with a call
/// to [send()](AsyncDumper::send).
///
/// ## Example
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::{AsyncWriteExt, Cursor};
///
/// use mbon::async_wrapper::AsyncDumper;
///
/// let mut writer = Cursor::new(vec![0u8; 5]);
/// let mut dumper = AsyncDumper::from(writer);
///
/// dumper.write(&15u32)?;
/// dumper.flush().await?;
///
/// assert_eq!(dumper.writer().into_inner(), b"i\x00\x00\x00\x0f");
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
pub struct AsyncDumper<R> {
    writer: R,
    dumper: Dumper<Vec<u8>>,
}

impl<R> From<R> for AsyncDumper<R>
where
    R: AsyncWriteExt + Unpin,
{
    fn from(writer: R) -> Self {
        Self {
            writer,
            dumper: Dumper::new(),
        }
    }
}

impl<R> AsyncDumper<R>
where
    R: AsyncWriteExt + Unpin,
{
    /// Turn the dumper into the underlying writer
    pub fn writer(self) -> R {
        self.writer
    }

    /// Get the underlying writer as a reference
    pub fn get_writer(&self) -> &R {
        &self.writer
    }

    /// Get the underlying writer as a mutable reference
    pub fn get_writer_mut(&mut self) -> &mut R {
        &mut self.writer
    }

    /// Try to send pending data to the async writer
    ///
    /// If all of the pending data has been sent, true will be returned.
    ///
    /// If you want to send all data, use [flush()](AsyncDumper::flush) instead.
    pub async fn send(&mut self) -> Result<bool> {
        let written = self.writer.write(self.dumper.get_writer()).await?;

        let buf = &self.dumper.get_writer()[written..];
        let all_done = buf.is_empty();

        // update the buffer to contain what hasn't been sent yet
        if all_done {
            self.dumper.get_writer_mut().clear();
        } else {
            let buf = buf.to_vec();
            drop(mem::replace(self.dumper.get_writer_mut(), buf));
        }

        Ok(all_done)
    }

    /// Send all pending data to the async writer
    ///
    /// The returned future will not complete until all data has been written to
    /// the writer.
    ///
    /// see [futures::AsyncWriteExt::write_all]
    pub async fn flush(&mut self) -> Result<()> {
        self.writer.write_all(self.dumper.get_writer()).await?;
        self.dumper.get_writer_mut().clear();
        Ok(())
    }

    /// Write a serializable object to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write()](crate::dumper::Dumper::write)
    #[inline]
    pub fn write<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.dumper.write(value)
    }

    /// Write a binary object to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_obj()](crate::dumper::Dumper::write_obj)
    #[inline]
    pub fn write_obj<T>(&mut self, value: &T) -> Result<()>
    where
        T: ObjectDump,
        <T as ObjectDump>::Error: std::error::Error + 'static,
    {
        self.dumper.write_obj(value)
    }

    /// Write a 64 bit integer to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_long()](crate::dumper::Dumper::write_long)
    #[inline]
    pub fn write_long(&mut self, val: i64) -> Result<()> {
        self.dumper.write_long(val)
    }

    /// Write a 32 bit integer to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_int()](crate::dumper::Dumper::write_int)
    #[inline]
    pub fn write_int(&mut self, val: i32) -> Result<()> {
        self.dumper.write_int(val)
    }

    /// Write a 16 bit integer to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_short()](crate::dumper::Dumper::write_short)
    #[inline]
    pub fn write_short(&mut self, val: i16) -> Result<()> {
        self.dumper.write_short(val)
    }

    /// Write a 8 bit integer to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_char()](crate::dumper::Dumper::write_char)
    #[inline]
    pub fn write_char(&mut self, val: i8) -> Result<()> {
        self.dumper.write_char(val)
    }

    /// Write a 32 bit IEEE754 float to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_float()](crate::dumper::Dumper::write_float)
    #[inline]
    pub fn write_float(&mut self, val: f32) -> Result<()> {
        self.dumper.write_float(val)
    }

    /// Write a 64 bit IEEE754 float to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_double()](crate::dumper::Dumper::write_double)
    #[inline]
    pub fn write_double(&mut self, val: f64) -> Result<()> {
        self.dumper.write_double(val)
    }

    /// Write a string of bytes to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_bytes()](crate::dumper::Dumper::write_bytes)
    #[inline]
    pub fn write_bytes(&mut self, val: impl AsRef<[u8]>) -> Result<()> {
        self.dumper.write_bytes(val)
    }

    /// Write a string to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_str()](crate::dumper::Dumper::write_str)
    #[inline]
    pub fn write_str(&mut self, val: impl AsRef<str>) -> Result<()> {
        self.dumper.write_str(val)
    }

    /// Write a binary object to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_object()](crate::dumper::Dumper::write_object)
    #[inline]
    pub fn write_object(&mut self, val: impl AsRef<[u8]>) -> Result<()> {
        self.dumper.write_object(val)
    }

    /// Write an indexed value to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_enum()](crate::dumper::Dumper::write_enum)
    #[inline]
    pub fn write_enum(&mut self, variant: u32, val: impl AsRef<Value>) -> Result<()> {
        self.dumper.write_enum(variant, val)
    }

    /// Write an null value to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_null()](crate::dumper::Dumper::write_null)
    #[inline]
    pub fn write_null(&mut self) -> Result<()> {
        self.dumper.write_null()
    }

    /// Write an list of values to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_list()](crate::dumper::Dumper::write_list)
    #[inline]
    pub fn write_list(&mut self, val: impl AsRef<Vec<Value>>) -> Result<()> {
        self.dumper.write_list(val)
    }

    /// Write an key, value map of values to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_map()](crate::dumper::Dumper::write_map)
    #[inline]
    pub fn write_map(&mut self, val: impl AsRef<Vec<(Value, Value)>>) -> Result<()> {
        self.dumper.write_map(val)
    }

    /// Write any value to the buffer.
    ///
    /// This will not send any data to the writer, use
    /// [flush()](AsyncDumper::flush) to write to the writer.
    ///
    /// see [Dumper::write_value()](crate::dumper::Dumper::write_value)
    #[inline]
    pub fn write_value(&mut self, val: impl AsRef<Value>) -> Result<()> {
        self.dumper.write_value(val)
    }
}

// TODO: Make a wrapper for Parser that can read a mark on its own, then read in
// The expected number of characters from the mark, then call
// Parser::next_data_value() with the data that was read
