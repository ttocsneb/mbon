//! # Async Wrappers for Dumper and Parser
//!
//! > You need to enable the feature `async` to use these implementations.
//!
//! Async wrappers are provided for [Dumper]: [AsyncDumper] and [Parser]:
//! [AsyncParser].
//!
//! [Dumper]: crate::dumper::Dumper
//! [Parser]: crate::parser::Parser

use std::io::SeekFrom;
use std::mem;

use crate::data::{Mark, Type, Value};
use crate::dumper::Dumper;
use crate::error::Result;
use crate::object::{ObjectDump, ObjectParse};
use crate::parser::Parser;

use async_recursion::async_recursion;
use byteorder::{BigEndian, ReadBytesExt};
use futures::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use serde::de::DeserializeOwned;
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
/// let writer = Cursor::new(vec![0u8; 5]);
/// let mut dumper = AsyncDumper::from(writer);
///
/// dumper.write(&15u32)?;
/// dumper.flush().await?;
///
/// assert_eq!(dumper.writer().into_inner(), b"i\x00\x00\x00\x0f");
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
#[derive(Debug)]
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

impl<R> AsRef<R> for AsyncDumper<R> {
    fn as_ref(&self) -> &R {
        &self.writer
    }
}

impl<R> AsMut<R> for AsyncDumper<R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.writer
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

/// A wrapper for [Parser](crate::parser::Parser).
///
/// AsyncParser reads from the reader into a buffer where Parser can parse the
/// requested data. Every request for data will ask for exactly what's needed
/// to perform the task.
///
/// ## Example
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::io::Cursor;
///
/// use mbon::async_wrapper::AsyncParser;
///
/// let reader = Cursor::new(b"i\x00\x00\x00\x0f");
/// let mut parser = AsyncParser::from(reader);
///
/// let val: u32 = parser.next().await?;
///
/// assert_eq!(val, 15);
/// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
/// ```
#[derive(Debug)]
pub struct AsyncParser<R>(R);

impl<R> From<R> for AsyncParser<R>
where
    R: AsyncReadExt + Unpin + Send,
{
    fn from(reader: R) -> Self {
        Self(reader)
    }
}

impl<R> AsRef<R> for AsyncParser<R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

impl<R> AsMut<R> for AsyncParser<R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.0
    }
}

impl<R> AsyncParser<R>
where
    R: AsyncReadExt + Unpin + Send,
{
    /// Turn the parser into the underlying reader
    #[inline]
    pub fn reader(self) -> R {
        self.0
    }

    /// Get the underlying reader as a reference
    #[inline]
    pub fn get_reader(&self) -> &R {
        &self.0
    }

    /// Get the underlying reader as a mutable reference
    #[inline]
    pub fn get_reader_mut(&mut self) -> &mut R {
        &mut self.0
    }

    /// Parse the next item in the parser.
    #[inline]
    pub async fn next<T>(&mut self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.next_value().await?.parse()
    }

    /// Parse the next custom object in the parser.
    ///
    /// This allows you to be able to parse custom binary data. A common usecase
    /// is to store a struct in a more compact form. You could also use object
    /// values to store a different format altogether.
    ///
    /// Note: the next value in the parser must be an Object
    ///
    /// see [Parser::next_obj()](crate::parser::Parser::next_obj)
    ///
    #[inline]
    pub async fn next_obj<T>(&mut self) -> Result<T>
    where
        T: ObjectParse,
        <T as ObjectParse>::Error: std::error::Error + 'static,
    {
        self.next_value().await?.parse_obj()
    }

    async fn next_size(&mut self) -> Result<usize> {
        let mut buf = [0u8; 4];
        self.0.read_exact(&mut buf).await?;
        Ok(buf.as_slice().read_u32::<BigEndian>()? as usize)
    }

    #[async_recursion]
    async fn next_mark(&mut self) -> Result<Mark> {
        // I don't particularly like this implementation as it redefines
        // next_mark, but I don't see another way to know the size of the data
        // without first getting the mark, and we can't get the size of the
        // mark from the prefix as some marks are recursive.
        let mut buf = [0u8; 1];
        self.0.read_exact(&mut buf).await?;
        let prefix = Type::from_prefix(buf[0])?;
        Ok(match prefix {
            Type::Long => Mark::Long,
            Type::Int => Mark::Int,
            Type::Short => Mark::Short,
            Type::Char => Mark::Char,
            Type::Float => Mark::Float,
            Type::Double => Mark::Double,
            Type::Bytes => Mark::Bytes(self.next_size().await?),
            Type::Str => Mark::Str(self.next_size().await?),
            Type::Object => Mark::Object(self.next_size().await?),
            Type::Enum => Mark::Enum(Box::new(self.next_mark().await?)),
            Type::Null => Mark::Null,
            Type::Array => {
                let mark = self.next_mark().await?;
                let len = self.next_size().await?;
                Mark::Array(len, Box::new(mark))
            }
            Type::List => Mark::List(self.next_size().await?),
            Type::Dict => {
                let kmark = self.next_mark().await?;
                let vmark = self.next_mark().await?;
                let len = self.next_size().await?;
                Mark::Dict(len, Box::new(kmark), Box::new(vmark))
            }
            Type::Map => Mark::Map(self.next_size().await?),
        })
    }

    /// Skip the next value in the parser.
    ///
    /// This will ignore the next value without parsing more than what's
    /// necessary.
    ///
    /// If the reader supports seeking, then it is preffered to use
    /// [`seek_next()`](AsyncParser::seek_next) instead.
    ///
    /// see [Parser::skip_next()](crate::parser::Parser::skip_next)
    pub async fn skip_next(&mut self) -> Result<()> {
        let mark = self.next_mark().await?;
        let mut buf = vec![0u8; mark.data_size()];
        self.0.read_exact(&mut buf).await?;
        Ok(())
    }

    /// Parse the next value in the parser.
    ///
    /// see [Parser::next_value()](crate::parser::Parser::next_value)
    pub async fn next_value(&mut self) -> Result<Value> {
        let mark = self.next_mark().await?;
        let mut buf = vec![0u8; mark.data_size()];
        self.0.read_exact(&mut buf).await?;

        let mut parser = Parser::from(&buf);
        parser.next_data_value(&mark)
    }
}

impl<R> AsyncParser<R>
where
    R: AsyncReadExt + AsyncSeekExt + Unpin + Send,
{
    /// Seek to the next value in the parser.
    ///
    /// This will efficiently skip the next value without reading more than
    /// what's necessary
    ///
    /// see [Parser::seek_next()](crate::parser::Parser::seek_next)
    pub async fn seek_next(&mut self) -> Result<()> {
        let mark = self.next_mark().await?;
        self.0
            .seek(SeekFrom::Current(mark.data_size() as i64))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::io::Cursor;

    #[test]
    fn test_parser() {
        futures::executor::block_on(async {
            let reader = Cursor::new(b"ac\x00\x00\x00\x04\x01\x02\x03\x04");
            let mut parser = AsyncParser::from(reader);

            let val: Vec<u8> = parser.next().await?;

            assert_eq!(val, vec![1, 2, 3, 4]);
            Ok::<(), Box<dyn std::error::Error>>(())
        })
        .unwrap();
    }

    #[test]
    fn test_seek() {
        futures::executor::block_on(async {
            let reader = Cursor::new(
                b"s\x00\x00\x00\x23This is a string I don't care abouti\x00\x00\x00\x20",
            );
            let mut parser = AsyncParser::from(reader);

            parser.seek_next().await?;
            let val: u32 = parser.next().await?;

            assert_eq!(val, 32);
            Ok::<(), Box<dyn std::error::Error>>(())
        })
        .unwrap();
    }

    #[test]
    fn test_skip() {
        futures::executor::block_on(async {
            let reader = Cursor::new(
                b"s\x00\x00\x00\x23This is a string I don't care abouti\x00\x00\x00\x20",
            );
            let mut parser = AsyncParser::from(reader);

            parser.skip_next().await?;
            let val: u32 = parser.next().await?;

            assert_eq!(val, 32);
            Ok::<(), Box<dyn std::error::Error>>(())
        })
        .unwrap();
    }
}
