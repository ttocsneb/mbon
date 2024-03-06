use enum_as_inner::EnumAsInner;
use maybe_async::maybe_async;
use std::{
    char::{self},
    io::SeekFrom,
    mem,
    ops::Deref,
    sync::Arc,
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
    pub(crate) async fn parse<R: Reader>(f: &mut R, l: &Size) -> MbonResult<Self> {
        let mut buf = vec![0u8; **l as usize];
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
    pub(crate) fn new(start: u64, l: &Size) -> MbonResult<Self> {
        let end = start + **l;
        Ok(List {
            items: Vec::new(),
            start,
            end,
        })
    }

    pub async fn fetch<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut PartialItem>> {
        if self.items.len() > index {
            return Ok(Some(&mut self.items[index]));
        }

        let mut location = match self.items.last() {
            Some(item) => item.location + item.mark.total_len(),
            None => self.start,
        };
        if location > self.end {
            return Err(MbonError::InvalidMark);
        }
        let mut len = self.items.len();

        loop {
            let (mark, pos) = client.parse_mark(location).await?;
            let item = PartialItem::new(mark, pos);
            location = item.location + item.mark.total_len();
            if location > self.end {
                return Err(MbonError::InvalidMark);
            }
            self.items.push(item);
            len += 1;

            if len == index + 1 {
                return Ok(Some(&mut self.items[index]));
            }
            if location == self.end {
                return Ok(None);
            }
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&PartialItem> {
        self.items.get(index)
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut PartialItem> {
        self.items.get_mut(index)
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
    pub fn new(start: u64, mark: Arc<Mark>, n: &Size) -> MbonResult<Self> {
        let items = vec![None; **n as usize];
        Ok(Array { items, mark, start })
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

    #[inline]
    pub fn get(&self, index: usize) -> Option<&Data> {
        self.items.get(index).map(|v| v.as_ref()).flatten()
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Data> {
        self.items.get_mut(index).map(|v| v.as_mut()).flatten()
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
    pub fn new(start: u64, key: Arc<Mark>, val: Arc<Mark>, n: &Size) -> MbonResult<Self> {
        let items = vec![(None, None); **n as usize];
        Ok(Self {
            items,
            key,
            val,
            start,
        })
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
    pub async fn fetch_key<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        self.fetch_nth(client, index * 2).await
    }

    #[inline]
    pub async fn fetch_val<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        index: usize,
    ) -> MbonResult<Option<&'t mut Data>> {
        self.fetch_nth(client, index * 2 + 1).await
    }

    pub async fn fetch_by_key<'t, E: MbonParserRead>(
        &'t mut self,
        client: &mut E,
        _key: &Data,
    ) -> MbonResult<Option<&'t mut Data>> {
        for i in 0..self.items.len() {
            if let Some(_k) = self.fetch_key(client, i).await? {
                todo!()
            }
        }

        todo!()
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
}

#[maybe_async]
impl Data {
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
            Mark::String(l) => Self::String(Str::parse(f, l).await?),
            Mark::Array(v, n) => {
                Self::Array(Array::new(f.stream_position().await?, v.clone(), &n)?)
            }
            Mark::List(l) => Self::List(List::new(f.stream_position().await?, l)?),
            Mark::Struct(k, v, n) => Self::Struct(Struct::new(
                f.stream_position().await?,
                k.clone(),
                v.clone(),
                &n,
            )?),
            Mark::Map(_) => todo!(),
            Mark::Enum(_, _) => todo!(),
            Mark::Space => todo!(),
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
            Data::List(_) => todo!(),
            Data::Array(_) => todo!(),
            Data::Struct(_) => todo!(),
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
        f.seek(SeekFrom::Start(self.location)).await?;
        let data = Data::parse(f, &self.mark).await?;
        self.data = Some(data);
        Ok(())
    }
}
