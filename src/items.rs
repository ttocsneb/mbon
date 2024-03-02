use enum_as_inner::EnumAsInner;

use crate::data::{self};

#[derive(Debug, PartialEq, Clone, EnumAsInner)]
pub enum Item {
    Null,
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Char(char),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Item>),
    Map(Vec<(Item, Item)>),
    Enum(u32, Box<Item>),
}

macro_rules! item_from {
    ($name:ident, $type:ty) => {
        impl From<$type> for Item {
            fn from(value: $type) -> Self {
                Self::$name(value)
            }
        }
    };
    ($name:ident, $type:ty, $value:ident: $expr:expr) => {
        impl From<$type> for Item {
            fn from($value: $type) -> Self {
                Self::$name($expr)
            }
        }
    };
}

item_from!(U8, u8);
item_from!(U8, data::U8, v: *v);
item_from!(U16, u16);
item_from!(U16, data::U16, v: *v);
item_from!(U32, u32);
item_from!(U32, data::U32, v:*v);
item_from!(U64, u64);
item_from!(U64, data::U64, v:*v);
item_from!(I8, i8);
item_from!(I8, data::I8, v:*v);
item_from!(I16, i16);
item_from!(I16, data::I16, v:*v);
item_from!(I32, i32);
item_from!(I32, data::I32, v:*v);
item_from!(I64, i64);
item_from!(I64, data::I64, v:*v);
item_from!(F32, f32);
item_from!(F32, data::F32, v:*v);
item_from!(F64, f64);
item_from!(F64, data::F64, v:*v);
item_from!(Char, char);
item_from!(Char, data::C8, v:*v as char);
item_from!(Bytes, Vec<u8>);
item_from!(List, Vec<Item>);
item_from!(Map, Vec<(Item, Item)>);
item_from!(String, data::Str, v:v.into());

impl<T> From<Option<T>> for Item
where
    T: Into<Item>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::Null,
        }
    }
}

impl<I> FromIterator<I> for Item
where
    I: Into<Item>,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = I>,
    {
        Self::List(iter.into_iter().map(|v| v.into()).collect())
    }
}

impl<K, V> FromIterator<(K, V)> for Item
where
    K: Into<Item>,
    V: Into<Item>,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (K, V)>,
    {
        Self::Map(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}
