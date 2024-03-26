use anyhow::anyhow;
use async_recursion::async_recursion;
use core::slice;
use enum_as_inner::EnumAsInner;
use maybe_async::maybe_async;
use std::{
    char::{self},
    io::{self, SeekFrom},
    mem,
    ops::Deref,
    sync::Arc,
    vec,
};

use crate::{
    engine::MbonParserRead,
    errors::{MbonError, MbonResult},
    marks::{Mark, Size},
    stream::{Reader, Seeker},
};

macro_rules! number_type {
    ($name:ident, $type:ty, $file:ident: $read:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name($type);
        impl Deref for $name {
            type Target = $type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl $name {
            #[maybe_async]
            pub(crate) async fn parse<R: Reader>($file: &mut R) -> MbonResult<Self> {
                let val = $read;
                Ok(Self(val))
            }

            pub fn value(&self) -> $type {
                self.0
            }
        }
    };
}

macro_rules! char_impl {
    ($name:ident) => {
        impl $name {
            #[inline]
            pub fn as_char(&self) -> Option<char> {
                char::from_u32(self.0 as u32)
            }
        }
    };
}

number_type!(U8,  u8,  f: f.read_u8().await?);
number_type!(U16, u16, f: f.read_u16_le().await?);
number_type!(U32, u32, f: f.read_u32_le().await?);
number_type!(U64, u64, f: f.read_u64_le().await?);
number_type!(I8,  i8,  f: f.read_i8().await?);
number_type!(I16, i16, f: f.read_i16_le().await?);
number_type!(I32, i32, f: f.read_i32_le().await?);
number_type!(I64, i64, f: f.read_i64_le().await?);
number_type!(F32, f32, f: f.read_f32_le().await?);
number_type!(F64, f64, f: f.read_f64_le().await?);
number_type!(C8,  u8,  f: f.read_u8().await?);
char_impl!(C8);
number_type!(C16, u16, f: f.read_u16_le().await?);
char_impl!(C16);
number_type!(C32, u32, f: f.read_u32_le().await?);
char_impl!(C32);

#[derive(Debug, Clone)]
pub struct Str(String);
impl Deref for Str {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<Str> for String {
    fn from(value: Str) -> Self {
        value.0
    }
}
impl Str {
    #[maybe_async]
    pub(crate) async fn parse<R: Reader>(f: &mut R, l: Size) -> MbonResult<Self> {
        let mut buf = vec![0u8; *l as usize];
        f.read_exact(buf.as_mut_slice()).await?;
        let val = String::from_utf8(buf).map_err(|err| MbonError::InvalidData(err.into()))?;
        Ok(Self(val))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct List {
    items: Vec<PartialItem>,
    start: u64,
    end: u64,
}

#[maybe_async]
impl List {
    pub(crate) async fn parse_full<R: Reader + Seeker>(f: &mut R, l: Size) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let mut this = List {
            items: Vec::new(),
            start,
            end: start + l.value(),
        };

        let mut pos = start;
        loop {
            let (mark, _) = Mark::parse(f).await?;
            let mut item = PartialItem::new(mark, pos);
            item.parse_data_full(f).await?;
            this.items.push(item);
            pos = f.stream_position().await?;
            if pos == this.end {
                break;
            }
            if pos > this.end {
                return Err(MbonError::InvalidData(anyhow!(
                    "Mark size and actual size do not match"
                )));
            }
        }
        Ok(this)
    }

    pub(crate) async fn parse<R: Seeker + Reader>(f: &mut R, l: Size) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let mut this = List {
            items: Vec::new(),
            start,
            end: start + l.value(),
        };

        let mut pos = start;
        loop {
            let (mark, _) = Mark::parse(f).await?;
            let item = PartialItem::new(mark, pos);
            this.items.push(item);
            pos = f.stream_position().await?;
            if pos == this.end {
                break;
            }
            if pos > this.end {
                return Err(MbonError::InvalidData(anyhow!(
                    "Mark size and actual size do not match"
                )));
            }
        }
        Ok(this)
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&PartialItem> {
        self.items.get(index)
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut PartialItem> {
        self.items.get_mut(index)
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, PartialItem> {
        self.items.iter()
    }

    #[inline]
    pub fn iter_mut<'a>(&'a mut self) -> slice::IterMut<'a, PartialItem> {
        self.items.iter_mut()
    }

    pub fn maybe_eq(&self, rhs: &Self) -> Option<bool> {
        if self.items.len() != rhs.items.len() {
            return Some(false);
        }
        for (lhs, rhs) in self.iter().zip(rhs.iter()) {
            if !lhs.maybe_eq(rhs)? {
                return Some(false);
            }
        }
        return Some(true);
    }
}

impl IntoIterator for List {
    type Item = PartialItem;

    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'t> IntoIterator for &'t List {
    type Item = &'t PartialItem;

    type IntoIter = slice::Iter<'t, PartialItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Array {
    items: Vec<Option<Data>>,
    pub mark: Arc<Mark>,
    start: u64,
}

#[maybe_async]
impl Array {
    pub(crate) fn new(start: u64, mark: Arc<Mark>, n: Size) -> MbonResult<Self> {
        let items = vec![None; *n as usize];
        Ok(Array { items, mark, start })
    }

    pub(crate) async fn parse_full<F: Reader + Seeker>(
        f: &mut F,
        mark: Arc<Mark>,
        n: Size,
    ) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let mut this = Self {
            items: vec![None; *n as usize],
            mark,
            start,
        };
        for v in &mut this.items {
            let value = Data::parse_full(f, &this.mark).await?;
            let _ = mem::replace(v, Some(value));
        }
        Ok(this)
    }

    pub async fn fetch<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        if self.items.len() <= index {
            return Ok(None);
        }

        if self.items[index].is_some() {
            return Ok(self.items[index].as_mut());
        }

        let len = self.mark.data_len();
        let location = self.start + len * (index as u64);
        let data = client.parse_data(&self.mark, location).await?;

        self.items[index] = Some(data);
        Ok(self.items[index].as_mut())
    }

    pub async fn fetch_all<'a, E: MbonParserRead>(&'a mut self, client: &mut E) -> MbonResult<()> {
        let len = self.mark.data_len();
        for (i, val) in self.items.iter_mut().enumerate() {
            if val.is_some() {
                continue;
            }
            let location = self.start + len * (i as u64);
            let data = client.parse_data(&self.mark, location).await?;
            let _ = mem::replace(val, Some(data));
        }
        Ok(())
    }

    pub async fn fetch_all_full<'a, E: MbonParserRead>(
        &'a mut self,
        client: &mut E,
    ) -> MbonResult<()> {
        let len = self.mark.data_len();
        for (i, val) in self.items.iter_mut().enumerate() {
            if val.is_some() {
                continue;
            }
            let location = self.start + len * (i as u64);
            let data = client.parse_data_full(&self.mark, location).await?;
            let _ = mem::replace(val, Some(data));
        }
        Ok(())
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&Data> {
        self.items.get(index).map(|v| v.as_ref()).flatten()
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Data> {
        self.items.get_mut(index).map(|v| v.as_mut()).flatten()
    }

    #[inline]
    pub fn iter<'a>(&'a self) -> slice::Iter<'a, Option<Data>> {
        self.items.iter()
    }

    #[inline]
    pub fn iter_mut<'a>(&'a mut self) -> slice::IterMut<'a, Option<Data>> {
        self.items.iter_mut()
    }

    pub fn maybe_eq(&self, rhs: &Self) -> Option<bool> {
        if self.items.len() != rhs.items.len() {
            return Some(false);
        }

        for (lhs, rhs) in self.iter().zip(rhs.iter()) {
            if let Some(lhs) = lhs {
                if let Some(rhs) = rhs {
                    if !lhs.maybe_eq(rhs)? {
                        return Some(false);
                    }
                }
            }
            return None;
        }

        Some(true)
    }
}
impl IntoIterator for Array {
    type Item = Option<Data>;

    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'t> IntoIterator for &'t Array {
    type Item = &'t Option<Data>;

    type IntoIter = slice::Iter<'t, Option<Data>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Struct {
    items: Vec<(Option<Data>, Option<Data>)>,
    pub key: Arc<Mark>,
    pub val: Arc<Mark>,
    start: u64,
}

#[maybe_async]
impl Struct {
    pub fn new(start: u64, key: Arc<Mark>, val: Arc<Mark>, n: Size) -> MbonResult<Self> {
        let items = vec![(None, None); *n as usize];
        Ok(Self {
            items,
            key,
            val,
            start,
        })
    }

    pub(crate) async fn parse_full<R: Reader + Seeker>(
        f: &mut R,
        key: Arc<Mark>,
        val: Arc<Mark>,
        n: Size,
    ) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let mut this = Self {
            items: vec![(None, None); n.value() as usize],
            key,
            val,
            start,
        };
        for (key, val) in &mut this.items {
            let key_data = Data::parse_full(f, &this.key).await?;
            let val_data = Data::parse_full(f, &this.val).await?;
            let _ = mem::replace(key, Some(key_data));
            let _ = mem::replace(val, Some(val_data));
        }
        Ok(this)
    }

    async fn fetch_nth<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        let item_i = index / 2;
        let korv = index & 0b1 != 0;

        let (k, v) = match self.items.get_mut(item_i) {
            Some(v) => v,
            None => return Ok(None),
        };
        let (val, mark) = match korv {
            true => (k, &self.key),
            false => (v, &self.val),
        };
        if let Some(val) = val {
            return Ok(Some(val));
        }
        let key_len = self.key.data_len();
        let val_len = self.val.data_len();
        let mut offset = (key_len + val_len) * item_i as u64;
        if !korv {
            offset += key_len;
        }

        let data = client.parse_data(mark, self.start + offset).await?;

        let _ = mem::replace(val, Some(data));

        Ok(val.as_mut())
    }

    #[inline]
    pub async fn fetch_nth_key<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        self.fetch_nth(client, index * 2).await
    }

    #[inline]
    pub async fn fetch_nth_val<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        self.fetch_nth(client, index * 2 + 1).await
    }

    pub async fn fetch_by_key<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        key: &Data,
    ) -> MbonResult<Option<&'t mut Data>> {
        for i in 0..self.items.len() {
            if let Some(k) = self.fetch_nth_key(client, i).await? {
                if let Some(true) = k.maybe_eq(key) {
                    return self.fetch_nth_val(client, i).await;
                }
            }
        }

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct Map {
    items: Vec<PartialItem>,
    start: u64,
    end: u64,
}

#[maybe_async]
impl Map {
    pub(crate) async fn parse<R: Reader + Seeker>(f: &mut R, l: Size) -> MbonResult<Self> {
        let list = List::parse(f, l).await?;
        Ok(Self {
            items: list.items,
            start: list.start,
            end: list.end,
        })
    }

    pub(crate) async fn parse_full<R: Reader + Seeker>(f: &mut R, l: Size) -> MbonResult<Self> {
        let list = List::parse_full(f, l).await?;
        Ok(Self {
            items: list.items,
            start: list.start,
            end: list.end,
        })
    }

    #[inline]
    pub fn get_nth_key<'a>(&'a self, i: usize) -> Option<&'a PartialItem> {
        let index = i * 2;
        self.items.get(index)
    }
    #[inline]
    pub fn get_nth_val<'a>(&'a self, i: usize) -> Option<&'a PartialItem> {
        let index = i * 2 + 1;
        self.items.get(index)
    }
    #[inline]
    pub fn get_nth_key_mut<'a>(&'a mut self, i: usize) -> Option<&'a mut PartialItem> {
        let index = i * 2;
        self.items.get_mut(index)
    }
    #[inline]
    pub fn get_nth_val_mut<'a>(&'a mut self, i: usize) -> Option<&'a mut PartialItem> {
        let index = i * 2 + 1;
        self.items.get_mut(index)
    }

    pub async fn get<'a>(&'a self, key: &PartialItem) -> Option<&'a PartialItem> {
        for i in 0..self.items.len() / 2 {
            if let Some(val) = self.get_nth_key(i) {
                if let Some(true) = key.maybe_eq(val) {
                    return self.get_nth_val(i);
                }
            }
        }
        None
    }
    pub async fn get_mut<'a>(&'a mut self, key: &PartialItem) -> Option<&'a mut PartialItem> {
        for i in 0..self.items.len() / 2 {
            if let Some(val) = self.get_nth_key(i) {
                if let Some(true) = key.maybe_eq(val) {
                    return self.get_nth_val_mut(i);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Enum<V> {
    variant: V,
    mark: Arc<Mark>,
    data: Option<Box<Data>>,
    start: u64,
}

#[maybe_async(AFIT)]
trait Variant<T> {
    async fn parse_variant<R: Reader + Seeker>(f: &mut R) -> io::Result<T>;
}

#[maybe_async(AFIT)]
impl Variant<u8> for Enum<u8> {
    async fn parse_variant<R: Reader + Seeker>(f: &mut R) -> io::Result<u8> {
        f.read_u8().await
    }
}
#[maybe_async(AFIT)]
impl Variant<u16> for Enum<u16> {
    async fn parse_variant<R: Reader + Seeker>(f: &mut R) -> io::Result<u16> {
        f.read_u16_le().await
    }
}
#[maybe_async(AFIT)]
impl Variant<u32> for Enum<u32> {
    async fn parse_variant<R: Reader + Seeker>(f: &mut R) -> io::Result<u32> {
        f.read_u32_le().await
    }
}

#[maybe_async]
#[allow(private_bounds)] // This should be okay since all functions in here are private
impl<T: Eq> Enum<T>
where
    Enum<T>: Variant<T>,
{
    async fn parse<R: Reader + Seeker>(f: &mut R, mark: Arc<Mark>) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let variant = Self::parse_variant(f).await?;
        Ok(Self {
            variant,
            mark,
            data: None,
            start,
        })
    }
    async fn parse_full<R: Reader + Seeker>(f: &mut R, mark: Arc<Mark>) -> MbonResult<Self> {
        let start = f.stream_position().await?;
        let variant = Self::parse_variant(f).await?;
        let data = Data::parse_full(f, &mark).await?;
        Ok(Self {
            variant,
            mark,
            data: Some(Box::new(data)),
            start,
        })
    }

    fn maybe_eq(&self, rhs: &Self) -> Option<bool> {
        if self.mark != rhs.mark {
            return Some(false);
        }
        if self.variant != rhs.variant {
            return Some(false);
        }
        if let Some(lhs) = &self.data {
            if let Some(rhs) = &rhs.data {
                return lhs.maybe_eq(rhs);
            }
        }
        if self.start == rhs.start {
            // Has to be the same if it has the same address
            return Some(true);
        }
        None
    }
}

#[derive(Debug, Clone, EnumAsInner)]
pub enum Data {
    Null,
    U8(U8),
    U16(U16),
    U32(U32),
    U64(U64),
    I8(I8),
    I16(I16),
    I32(I32),
    I64(I64),
    F32(F32),
    F64(F64),
    C8(C8),
    C16(C16),
    C32(C32),
    String(Str),
    List(List),
    Array(Array),
    Struct(Struct),
    Map(Map),
    Enum8(Enum<u8>),
    Enum16(Enum<u16>),
    Enum32(Enum<u32>),
    Space,
}

#[maybe_async]
impl Data {
    #[async_recursion]
    pub(crate) async fn parse_full<R: Reader + Seeker>(f: &mut R, mark: &Mark) -> MbonResult<Self> {
        Ok(match mark {
            Mark::Null => Self::Null,
            Mark::Unsigned(b) => match b {
                1 => Self::U8(U8::parse(f).await?),
                2 => Self::U16(U16::parse(f).await?),
                4 => Self::U32(U32::parse(f).await?),
                8 => Self::U64(U64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Signed(b) => match b {
                1 => Self::I8(I8::parse(f).await?),
                2 => Self::I16(I16::parse(f).await?),
                4 => Self::I32(I32::parse(f).await?),
                8 => Self::I64(I64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Float(b) => match b {
                4 => Self::F32(F32::parse(f).await?),
                8 => Self::F64(F64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Char(b) => match b {
                1 => Self::C8(C8::parse(f).await?),
                2 => Self::C16(C16::parse(f).await?),
                4 => Self::C32(C32::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::String(l) => Self::String(Str::parse(f, *l).await?),
            Mark::Array(v, n) => Self::Array(Array::parse_full(f, v.clone(), *n).await?),
            Mark::List(l) => Self::List(List::parse_full(f, *l).await?),
            Mark::Struct(k, v, n) => {
                Self::Struct(Struct::parse_full(f, k.clone(), v.clone(), *n).await?)
            }
            Mark::Map(l) => Self::Map(Map::parse_full(f, *l).await?),
            Mark::Enum(b, m) => match b {
                1 => Self::Enum8(Enum::parse_full(f, m.clone()).await?),
                2 => Self::Enum16(Enum::parse_full(f, m.clone()).await?),
                4 => Self::Enum32(Enum::parse_full(f, m.clone()).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Space => Self::Space,
            Mark::Padding(_) => todo!(),
            Mark::Pointer(_) => todo!(),
            Mark::Rc(_, _) => todo!(),
            Mark::Heap(_) => todo!(),
        })
    }
    pub(crate) async fn parse<R: Reader + Seeker>(f: &mut R, mark: &Mark) -> MbonResult<Self> {
        Ok(match mark {
            Mark::Null => Self::Null,
            Mark::Unsigned(b) => match b {
                1 => Self::U8(U8::parse(f).await?),
                2 => Self::U16(U16::parse(f).await?),
                4 => Self::U32(U32::parse(f).await?),
                8 => Self::U64(U64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Signed(b) => match b {
                1 => Self::I8(I8::parse(f).await?),
                2 => Self::I16(I16::parse(f).await?),
                4 => Self::I32(I32::parse(f).await?),
                8 => Self::I64(I64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Float(b) => match b {
                4 => Self::F32(F32::parse(f).await?),
                8 => Self::F64(F64::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Char(b) => match b {
                1 => Self::C8(C8::parse(f).await?),
                2 => Self::C16(C16::parse(f).await?),
                4 => Self::C32(C32::parse(f).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::String(l) => Self::String(Str::parse(f, *l).await?),
            Mark::Array(v, n) => {
                Self::Array(Array::new(f.stream_position().await?, v.clone(), *n)?)
            }
            Mark::List(l) => Self::List(List::parse(f, *l).await?),
            Mark::Struct(k, v, n) => Self::Struct(Struct::new(
                f.stream_position().await?,
                k.clone(),
                v.clone(),
                *n,
            )?),
            Mark::Map(l) => Self::Map(Map::parse(f, *l).await?),
            Mark::Enum(b, m) => match b {
                1 => Self::Enum8(Enum::parse(f, m.clone()).await?),
                2 => Self::Enum16(Enum::parse(f, m.clone()).await?),
                4 => Self::Enum32(Enum::parse(f, m.clone()).await?),
                _ => return Err(MbonError::InvalidMark),
            },
            Mark::Space => Self::Space,
            Mark::Padding(_) => todo!(),
            Mark::Pointer(_) => todo!(),
            Mark::Rc(_, _) => todo!(),
            Mark::Heap(_) => todo!(),
        })
    }

    pub fn maybe_eq(&self, other: &Self) -> Option<bool> {
        match self {
            Data::Null => Some(other.is_null()),
            Data::U8(l) => Some(other.as_u8().map(|r| **l == **r).unwrap_or(false)),
            Data::U16(l) => Some(other.as_u16().map(|r| **l == **r).unwrap_or(false)),
            Data::U32(l) => Some(other.as_u32().map(|r| **l == **r).unwrap_or(false)),
            Data::U64(l) => Some(other.as_u64().map(|r| **l == **r).unwrap_or(false)),
            Data::I8(l) => Some(other.as_i8().map(|r| **l == **r).unwrap_or(false)),
            Data::I16(l) => Some(other.as_i16().map(|r| **l == **r).unwrap_or(false)),
            Data::I32(l) => Some(other.as_i32().map(|r| **l == **r).unwrap_or(false)),
            Data::I64(l) => Some(other.as_i64().map(|r| **l == **r).unwrap_or(false)),
            Data::F32(l) => Some(other.as_f32().map(|r| **l == **r).unwrap_or(false)),
            Data::F64(l) => Some(other.as_f64().map(|r| **l == **r).unwrap_or(false)),
            Data::C8(l) => Some(other.as_c8().map(|r| **l == **r).unwrap_or(false)),
            Data::C16(l) => Some(other.as_c16().map(|r| **l == **r).unwrap_or(false)),
            Data::C32(l) => Some(other.as_c32().map(|r| **l == **r).unwrap_or(false)),
            Data::String(l) => Some(other.as_string().map(|r| **l == **r).unwrap_or(false)),
            Data::List(l) => other
                .as_list()
                .map(|r| l.maybe_eq(r))
                .unwrap_or(Some(false)),
            Data::Array(_) => todo!(),
            Data::Struct(_) => todo!(),
            Data::Map(_) => todo!(),
            Data::Enum8(l) => other
                .as_enum8()
                .map(|r| l.maybe_eq(r))
                .unwrap_or(Some(false)),
            Data::Enum16(l) => other
                .as_enum16()
                .map(|r| l.maybe_eq(r))
                .unwrap_or(Some(false)),
            Data::Enum32(l) => other
                .as_enum32()
                .map(|r| l.maybe_eq(r))
                .unwrap_or(Some(false)),
            Data::Space => Some(other.is_space()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PartialItem {
    pub mark: Mark,
    pub data: Option<Data>,
    location: u64,
}

#[maybe_async]
impl PartialItem {
    pub fn new(mark: Mark, location: u64) -> Self {
        Self {
            mark,
            location,
            data: None,
        }
    }

    pub(crate) async fn parse_data<R: Reader + Seeker>(&mut self, f: &mut R) -> MbonResult<()> {
        f.seek(SeekFrom::Start(self.location + self.mark.mark_len()))
            .await?;
        let data = Data::parse(f, &self.mark).await?;
        self.data = Some(data);
        Ok(())
    }

    pub async fn parse_data_full<R: Reader + Seeker>(&mut self, f: &mut R) -> MbonResult<()> {
        f.seek(SeekFrom::Start(self.location + self.mark.mark_len()))
            .await?;
        let data = Data::parse_full(f, &self.mark).await?;
        self.data = Some(data);
        Ok(())
    }

    pub fn maybe_eq(&self, rhs: &Self) -> Option<bool> {
        if self.mark != rhs.mark {
            return Some(false);
        }
        if let Some(lhs) = &self.data {
            if let Some(rhs) = &rhs.data {
                return lhs.maybe_eq(rhs);
            }
        }
        if self.location == rhs.location {
            return Some(true);
        }
        None
    }
}
