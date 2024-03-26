#[cfg(feature = "sync")]
use std::io::{self, Read, Seek, Write};

#[cfg(feature = "sync")]
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

#[cfg(feature = "async-tokio")]
pub trait Reader: AsyncRead + AsyncReadExt + Unpin + Send {}
#[cfg(feature = "async-tokio")]
pub trait Writer: AsyncWrite + AsyncWriteExt + Unpin + Send {}
#[cfg(feature = "async-tokio")]
pub trait Seeker: AsyncSeek + AsyncSeekExt + Unpin + Send {}

#[cfg(feature = "async-tokio")]
impl<F: AsyncReadExt + Unpin + Send> Reader for F {}
#[cfg(feature = "async-tokio")]
impl<F: AsyncWriteExt + Unpin + Send> Writer for F {}
#[cfg(feature = "async-tokio")]
impl<F: AsyncSeekExt + Unpin + Send> Seeker for F {}

#[cfg(feature = "sync")]
pub trait Reader: Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        ReadBytesExt::read_u8(self)
    }
    fn read_i8(&mut self) -> io::Result<i8> {
        ReadBytesExt::read_i8(self)
    }
    fn read_u16_le(&mut self) -> io::Result<u16> {
        ReadBytesExt::read_u16::<LittleEndian>(self)
    }
    fn read_i16_le(&mut self) -> io::Result<i16> {
        ReadBytesExt::read_i16::<LittleEndian>(self)
    }
    fn read_u32_le(&mut self) -> io::Result<u32> {
        ReadBytesExt::read_u32::<LittleEndian>(self)
    }
    fn read_i32_le(&mut self) -> io::Result<i32> {
        ReadBytesExt::read_i32::<LittleEndian>(self)
    }
    fn read_u64_le(&mut self) -> io::Result<u64> {
        ReadBytesExt::read_u64::<LittleEndian>(self)
    }
    fn read_i64_le(&mut self) -> io::Result<i64> {
        ReadBytesExt::read_i64::<LittleEndian>(self)
    }
    fn read_f32_le(&mut self) -> io::Result<f32> {
        ReadBytesExt::read_f32::<LittleEndian>(self)
    }
    fn read_f64_le(&mut self) -> io::Result<f64> {
        ReadBytesExt::read_f64::<LittleEndian>(self)
    }
}
#[cfg(feature = "sync")]
pub trait Writer: Write {
    fn write_u8(&mut self, val: u8) -> io::Result<()> {
        WriteBytesExt::write_u8(self, val)
    }
    fn write_i8(&mut self, val: i8) -> io::Result<()> {
        WriteBytesExt::write_i8(self, val)
    }
    fn write_u16_le(&mut self, val: u16) -> io::Result<()> {
        WriteBytesExt::write_u16::<LittleEndian>(self, val)
    }
    fn write_i16_le(&mut self, val: i16) -> io::Result<()> {
        WriteBytesExt::write_i16::<LittleEndian>(self, val)
    }
    fn write_u32_le(&mut self, val: u32) -> io::Result<()> {
        WriteBytesExt::write_u32::<LittleEndian>(self, val)
    }
    fn write_i32_le(&mut self, val: i32) -> io::Result<()> {
        WriteBytesExt::write_i32::<LittleEndian>(self, val)
    }
    fn write_u64_le(&mut self, val: u64) -> io::Result<()> {
        WriteBytesExt::write_u64::<LittleEndian>(self, val)
    }
    fn write_i64_le(&mut self, val: i64) -> io::Result<()> {
        WriteBytesExt::write_i64::<LittleEndian>(self, val)
    }
    fn write_f32_le(&mut self, val: f32) -> io::Result<()> {
        WriteBytesExt::write_f32::<LittleEndian>(self, val)
    }
    fn write_f64_le(&mut self, val: f64) -> io::Result<()> {
        WriteBytesExt::write_f64::<LittleEndian>(self, val)
    }
}
#[cfg(feature = "sync")]
pub trait Seeker: Seek {}

#[cfg(feature = "sync")]
impl<F: Read> Reader for F {}
#[cfg(feature = "sync")]
impl<F: Write> Writer for F {}
#[cfg(feature = "sync")]
impl<F: Seek> Seeker for F {}
