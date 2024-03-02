//! [Mark]

use std::{
    io,
    io::{Read, Write},
    ops::Deref,
    slice,
    sync::Arc,
};

use byteorder::ReadBytesExt;
use enum_as_inner::EnumAsInner;

use crate::errors::{MbonError, MbonResult};

/// Size indicator for marks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size(pub u64);

impl Deref for Size {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u64> for Size {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Size> for u64 {
    fn from(value: Size) -> Self {
        value.0
    }
}

impl Size {
    /// Parse a size from a reader
    ///
    /// This expects a dynamically sized Size indicator from _insert link to
    /// spec_.
    pub fn parse<R: Read>(f: &mut R) -> MbonResult<(Self, usize)> {
        let mut value = 0;
        let mut read = 0;

        let mut i = 0;
        loop {
            let b = f.read_u8()?;
            let v = (b & 0b0111_1111) as u64;
            if i == 9 && b > 1 {
                // 9 * 7 + 1 == 64
                // If the size is bigger than 64 bits, then return an error
                return Err(MbonError::InvalidMark);
            }
            value |= v << (7 * i);
            read += 1;
            if (b & 0b1000_0000) == 0 {
                break;
            }
            i += 1;
        }

        Ok((Self(value), read))
    }

    /// Write the size to a writer
    ///
    /// This will write a dynamically sized Size indicator from _insert link to
    /// spec_.
    pub fn write<W: Write>(&self, f: &mut W) -> io::Result<usize> {
        let mut value = self.0;
        let mut written = 0;
        while self.0 > 0 {
            let mut v = (value & 0b0111_1111) as u8;
            value = value >> 7;
            if value > 0 {
                v |= 0b1000_0000;
            }
            f.write_all(slice::from_ref(&v))?;
            written += 1;
        }
        Ok(written)
    }

    /// Get the value of the size
    #[inline]
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Get the number of bytes that the size would be represented by
    pub fn len(&self) -> u64 {
        let mut written = 0;
        let mut value = self.0;
        while self.0 > 0 {
            value = value >> 7;
            written += 1;
        }
        written
    }
}

/// Describes an Mbon item
#[derive(Debug, Clone, PartialEq, Eq, EnumAsInner)]
pub enum Mark {
    Null,
    Unsigned(u8),
    Signed(u8),
    Float(u8),
    Char(u8),
    String(Size),
    Array(Arc<Mark>, Size),
    List(Size),
    Struct(Arc<Mark>, Arc<Mark>, Size),
    Map(Size),
    Enum(u8, Arc<Mark>),
    Space,
    Padding(Size),
    Pointer(u8),
    Rc(u8, Arc<Mark>),
    Heap(Size),
}

fn len_b(id: u8) -> u8 {
    1 << (id & 0b11)
}

fn get_b(v: u8) -> u8 {
    match v {
        8 => 3,
        4 => 2,
        2 => 1,
        1 => 0,
        _ => 0,
    }
}

const NULL_ID: u8 = 0xc0;
const UNSIGNED_ID: u8 = 0x64;
const SIGNED_ID: u8 = 0x68;
const FLOAT_ID: u8 = 0x6c;
const CHAR_ID: u8 = 0x70;
const STRING_ID: u8 = 0x54;
const ARRAY_ID: u8 = 0x40;
const LIST_ID: u8 = 0x44;
const STRUCT_ID: u8 = 0x48;
const MAP_ID: u8 = 0x4c;
const ENUM_ID: u8 = 0x74;
const SPACE_ID: u8 = 0x80;
const PADDING_ID: u8 = 0x04;
const POINTER_ID: u8 = 0x28;
const RC_ID: u8 = 0x2c;
const HEAP_ID: u8 = 0x10;

impl Mark {
    /// Get the binary id of the mark
    pub fn id(&self) -> u8 {
        match self {
            Mark::Null => NULL_ID,
            Mark::Unsigned(v) => get_b(*v) | UNSIGNED_ID,
            Mark::Signed(v) => get_b(*v) | SIGNED_ID,
            Mark::Float(v) => get_b(*v) | FLOAT_ID,
            Mark::Char(v) => get_b(*v) | CHAR_ID,
            Mark::String(_) => STRING_ID,
            Mark::Array(_, _) => ARRAY_ID,
            Mark::List(_) => LIST_ID,
            Mark::Struct(_, _, _) => STRUCT_ID,
            Mark::Map(_) => MAP_ID,
            Mark::Enum(v, _) => get_b(*v) | ENUM_ID,
            Mark::Space => SPACE_ID,
            Mark::Padding(_) => PADDING_ID,
            Mark::Pointer(v) => get_b(*v) | POINTER_ID,
            Mark::Rc(v, _) => get_b(*v) | RC_ID,
            Mark::Heap(_) => HEAP_ID,
        }
    }

    /// Parse a mark from a reader
    pub fn parse<R: Read>(f: &mut R) -> MbonResult<(Self, usize)> {
        let id = f.read_u8()?;
        let mut len = 1;
        let mark = match id & 0b1111_1100 {
            NULL_ID => Self::Null,
            UNSIGNED_ID => Self::Unsigned(len_b(id)),
            SIGNED_ID => Self::Signed(len_b(id)),
            FLOAT_ID => Self::Float(len_b(id)),
            CHAR_ID => Self::Char(len_b(id)),
            STRING_ID => {
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::String(size)
            }
            ARRAY_ID => {
                let (val, r) = Self::parse(f)?;
                len += r;
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::Array(Arc::new(val), size)
            }
            LIST_ID => {
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::List(size)
            }
            STRUCT_ID => {
                let (key, r) = Self::parse(f)?;
                len += r;
                let (val, r) = Self::parse(f)?;
                len += r;
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::Struct(Arc::new(key), Arc::new(val), size)
            }
            MAP_ID => {
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::Map(size)
            }
            ENUM_ID => {
                let (mark, r) = Self::parse(f)?;
                len += r;
                Self::Enum(len_b(id), Arc::new(mark))
            }
            SPACE_ID => Self::Space,
            PADDING_ID => {
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::Padding(size)
            }
            POINTER_ID => Self::Pointer(len_b(id)),
            RC_ID => {
                let (mark, r) = Self::parse(f)?;
                len += r;
                Self::Rc(len_b(id), Arc::new(mark))
            }
            HEAP_ID => {
                let (size, r) = Size::parse(f)?;
                len += r;
                Self::Heap(size)
            }
            _ => return Err(MbonError::InvalidMark),
        };
        Ok((mark, len))
    }

    /// Write the mark to a writer
    pub fn write<W: Write>(&self, f: &mut W) -> io::Result<usize> {
        f.write_all(slice::from_ref(&self.id()))?;
        let mut written = 1;
        match self {
            Mark::String(l) => {
                written += l.write(f)?;
            }
            Mark::Array(v, n) => {
                written += v.write(f)?;
                written += n.write(f)?;
            }
            Mark::List(l) => {
                written += l.write(f)?;
            }
            Mark::Struct(k, v, n) => {
                written += k.write(f)?;
                written += v.write(f)?;
                written += n.write(f)?;
            }
            Mark::Map(l) => {
                written += l.write(f)?;
            }
            Mark::Enum(_, v) => {
                written += v.write(f)?;
            }
            Mark::Padding(l) => {
                written += l.write(f)?;
            }
            Mark::Rc(_, v) => {
                written += v.write(f)?;
            }
            Mark::Heap(l) => {
                written += l.write(f)?;
            }
            _ => {}
        }

        Ok(written)
    }

    /// Write the mark to a byte buffer
    #[inline]
    pub fn write_to_buf(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.write(&mut buf)?;
        Ok(buf)
    }

    /// Get the length of the data the mark represents
    pub fn data_len(&self) -> u64 {
        match self {
            Mark::Null => 0,
            Mark::Unsigned(b) => *b as u64,
            Mark::Signed(b) => *b as u64,
            Mark::Float(b) => *b as u64,
            Mark::Char(b) => *b as u64,
            Mark::String(l) => **l,
            Mark::Array(v, n) => v.data_len() * **n,
            Mark::List(l) => **l,
            Mark::Struct(k, v, n) => (k.data_len() + v.data_len()) * **n,
            Mark::Map(l) => **l,
            Mark::Enum(b, v) => *b as u64 + v.data_len(),
            Mark::Space => 0,
            Mark::Padding(l) => **l,
            Mark::Pointer(b) => *b as u64,
            Mark::Rc(b, v) => *b as u64 + v.data_len(),
            Mark::Heap(l) => **l,
        }
    }

    /// Get the length of the mark
    pub fn mark_len(&self) -> u64 {
        1 + match self {
            Mark::String(l) => l.len(),
            Mark::Array(v, n) => v.mark_len() + n.len(),
            Mark::List(l) => l.len(),
            Mark::Struct(k, v, n) => k.mark_len() + v.mark_len() + n.len(),
            Mark::Map(l) => l.len(),
            Mark::Enum(_, v) => v.mark_len(),
            Mark::Padding(l) => l.len(),
            Mark::Rc(_, v) => v.mark_len(),
            Mark::Heap(l) => l.len(),
            _ => 0,
        }
    }

    /// Get the length of the mark and data combined
    #[inline]
    pub fn total_len(&self) -> u64 {
        self.data_len() + self.mark_len()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_simple_parse() {
        let mut buf: &[u8] = &[0xc0, 0x64, 0x32];
        let (mark, read) = Mark::parse(&mut buf).unwrap();
        assert_eq!(read, 1);
        assert_eq!(mark.is_null(), true);

        let (mark, read) = Mark::parse(&mut buf).unwrap();
        assert_eq!(read, 1);
        assert_eq!(mark.is_unsigned(), true);
        if let Mark::Unsigned(b) = mark {
            assert_eq!(b, 1);
        } else {
            unreachable!();
        }

        let err = Mark::parse(&mut buf).expect_err("Expected InvalidMark error");
        assert_eq!(err.is_invalid_mark(), true);
    }

    #[test]
    fn test_size_parse() {
        let mut buf: &[u8] = &[0x32, 0x80, 0x31];

        let (size, read) = Size::parse(&mut buf).unwrap();
        assert_eq!(read, 1);
        assert_eq!(*size, 0x32);

        let (size, read) = Size::parse(&mut buf).unwrap();
        assert_eq!(read, 2);
        assert_eq!(*size, 0x1880);
    }
}
