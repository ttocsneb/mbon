//! Internal data structs
//!
//! Here, you'll find [Value], [Mark], and [Type].

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    object::{ObjectDump, ObjectParse},
};

use self::{de::ValueDe, ser::ValueSer};

pub mod de;
pub mod ser;

/// The basic unit for data in binary save data.
///
/// This is used as an intermidiate object for dumping/loading binary data. You
/// will generally not need to use this struct.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Long(i64),
    Int(i32),
    Short(i16),
    Char(i8),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    Str(String),
    Object(Vec<u8>),
    Enum(u32, Box<Value>),
    Null,
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

impl Value {
    /// Get the type of this value.
    ///
    /// This will return the actual type that would be stored when the value is
    /// converted into binary.
    ///
    /// ```
    /// use mbon::data::{Value, Type};
    ///
    /// assert_eq!(Value::Long(32).get_type(), Type::Long);
    ///
    /// assert_eq!(
    ///     Value::List(vec![Value::Int(64), Value::Int(12)]).get_type(),
    ///     Type::Array
    /// );
    ///
    /// assert_eq!(
    ///     Value::List(vec![Value::Int(64), Value::Short(12)]).get_type(),
    ///     Type::List
    /// );
    /// ```
    pub fn get_type(&self) -> Type {
        match self {
            Value::Long(_) => Type::Long,
            Value::Int(_) => Type::Int,
            Value::Short(_) => Type::Short,
            Value::Char(_) => Type::Char,
            Value::Float(_) => Type::Float,
            Value::Double(_) => Type::Double,
            Value::Bytes(_) => Type::Bytes,
            Value::Str(_) => Type::Str,
            Value::Object(_) => Type::Object,
            Value::Enum(_, _) => Type::Enum,
            Value::Null => Type::Null,
            Value::List(v) => {
                if Self::can_be_array(v) {
                    Type::Array
                } else {
                    Type::List
                }
            }
            Value::Map(v) => {
                if Self::can_be_dict(v) {
                    Type::Dict
                } else {
                    Type::Map
                }
            }
        }
    }

    /// Parse a struct from this value
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let foo: u32 = Value::Int(345).parse().unwrap();
    /// assert_eq!(foo, 345);
    /// ```
    #[inline]
    pub fn parse<'t, T>(&'t self) -> Result<T, Error>
    where
        T: Deserialize<'t>,
    {
        T::deserialize(ValueDe::new(&self))
    }

    /// Dump a struct into a value
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let obj: u32 = 345;
    /// let val = Value::dump(obj).unwrap();
    ///
    /// if let Value::Int(v) = val {
    ///     assert_eq!(v, 345);
    /// } else {
    ///     panic!("val is not an Int");
    /// }
    /// ```
    #[inline]
    pub fn dump<T>(value: T) -> Result<Self, Error>
    where
        T: Serialize,
    {
        value.serialize(&mut ValueSer)
    }

    /// Parse an object from this value
    ///
    /// This will attempt to parse an Object only if the Value is an Object
    /// type.
    ///
    /// ```
    /// use mbon::object::ObjectParse;
    /// use mbon::parser::Parser;
    /// use mbon::error::Error;
    /// use mbon::data::Value;
    ///
    /// struct Foo {
    ///     a: i32,
    ///     b: String,
    /// }
    ///
    /// impl ObjectParse for Foo {
    ///     type Error = Error;
    ///
    ///     fn parse_object(object: &[u8]) -> Result<Self, Self::Error> {
    ///         let mut parser = Parser::from(object);
    ///         let a = parser.next()?;
    ///         let b = parser.next()?;
    ///         Ok(Self { a, b })
    ///     }
    /// }
    ///
    /// let val = Value::Object(b"i\x00\x00\x32\x40s\x00\x00\x00\x05Hello".to_vec());
    /// let foo: Foo = val.parse_obj().unwrap();
    ///
    /// assert_eq!(foo.a, 0x3240);
    /// assert_eq!(foo.b, "Hello");
    /// ```
    pub fn parse_obj<T>(&self) -> Result<T, Error>
    where
        T: ObjectParse,
        <T as ObjectParse>::Error: std::error::Error + 'static,
    {
        if let Value::Object(data) = self {
            Error::from_res(T::parse_object(data))
        } else {
            Err(Error::Expected(Type::Object))
        }
    }

    /// Dump an object into this value
    ///
    /// ```
    /// use mbon::object::ObjectDump;
    /// use mbon::dumper::Dumper;
    /// use mbon::error::Error;
    /// use mbon::data::Value;
    ///
    /// struct Foo {
    ///     a: i32,
    ///     b: String,
    /// }
    ///
    /// impl ObjectDump for Foo {
    ///     type Error = Error;
    ///
    ///     fn dump_object(&self) -> Result<Vec<u8>, Self::Error> {
    ///         let mut dumper = Dumper::new();
    ///         dumper.write(&self.a);
    ///         dumper.write(&self.b);
    ///         Ok(dumper.writer())
    ///     }
    /// }
    ///
    /// let foo = Foo { a: 0x3240, b: "Hello".to_owned() };
    /// let val = Value::dump_obj(foo).unwrap();
    ///
    /// if let Value::Object(v) = val {
    ///     assert_eq!(v, b"i\x00\x00\x32\x40s\x00\x00\x00\x05Hello");
    /// } else {
    ///     panic!("val is not an Object");
    /// }
    /// ```
    #[inline]
    pub fn dump_obj<T>(value: T) -> Result<Self, Error>
    where
        T: ObjectDump,
        <T as ObjectDump>::Error: std::error::Error + 'static,
    {
        let data = Error::from_res(value.dump_object())?;
        Ok(Value::Object(data))
    }

    /// Get the total size in bytes that this value uses in binary form
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let value = Value::Int(42);
    ///
    /// assert_eq!(value.size(), 5);
    /// ```
    #[inline]
    pub fn size(&self) -> usize {
        Mark::from(self).size()
    }

    /// Get the size in bytes that the data will use in binary form
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let value = Value::Int(42);
    ///
    /// assert_eq!(value.data_size(), 4);
    /// ```
    #[inline]
    pub fn data_size(&self) -> usize {
        Mark::from(self).data_size()
    }

    /// Get the size in bytes that the mark will use in binary form
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let value = Value::Int(42);
    ///
    /// assert_eq!(value.mark_size(), 1);
    /// ```
    #[inline]
    pub fn mark_size(&self) -> usize {
        Mark::from(self).mark_size()
    }

    /// Check if a list can be stored as an array
    ///
    /// If all elements in the list have the same mark, then the list can be an
    /// array.
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let array = vec![Value::Int(32), Value::Int(42)];
    /// assert_eq!(Value::can_be_array(&array), true);
    ///
    /// let list = vec![Value::Int(32), Value::Char(42)];
    /// assert_eq!(Value::can_be_array(&list), false);
    /// ```
    pub fn can_be_array<'t, I>(list: I) -> bool
    where
        I: IntoIterator<Item = &'t Value>,
    {
        let mut iter = list.into_iter();
        if let Some(first) = iter.next() {
            let first_mark = Mark::from_value(first);

            for val in iter {
                let mark = Mark::from_value(val);
                if mark != first_mark {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// Check if a map can be stored as a dict
    ///
    /// If each key-value pair uses the same marks then the map can be a dict.
    ///
    /// ```
    /// use mbon::data::Value;
    ///
    /// let dict = vec![
    ///     (Value::Str("a".to_owned()), Value::Int(32)),
    ///     (Value::Str("b".to_owned()), Value::Int(42)),
    /// ];
    /// assert_eq!(Value::can_be_dict(&dict), true);
    ///
    /// let map = vec![
    ///     (Value::Str("a".to_owned()), Value::Int(32)),
    ///     (Value::Str("hello".to_owned()), Value::Int(42)),
    /// ];
    /// assert_eq!(Value::can_be_dict(&map), false);
    /// ```
    pub fn can_be_dict<'t, I>(map: I) -> bool
    where
        I: IntoIterator<Item = &'t (Value, Value)>,
    {
        let mut iter = map.into_iter();
        if let Some((first_k, first_v)) = iter.next() {
            let key_mark: Mark = first_k.into();
            let val_mark: Mark = first_v.into();

            for (k, v) in iter {
                let km: Mark = k.into();
                let vm: Mark = v.into();
                if km != key_mark || vm != val_mark {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
}

impl AsRef<Value> for Value {
    fn as_ref(&self) -> &Value {
        &self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Mark {
    Long,
    Int,
    Short,
    Char,
    Float,
    Double,
    Bytes(usize),
    Str(usize),
    Object(usize),
    Enum(Box<Mark>),
    Null,
    Array(usize, Box<Mark>),
    List(usize),
    Dict(usize, Box<Mark>, Box<Mark>),
    Map(usize),
}

impl Mark {
    /// Get the size in bytes that the mark will use in binary form
    ///
    /// ```
    /// use mbon::data::Mark;
    ///
    /// assert_eq!(Mark::Int.mark_size(), 1);
    /// ```
    pub fn mark_size(&self) -> usize {
        match self {
            Mark::Long => 1,
            Mark::Int => 1,
            Mark::Short => 1,
            Mark::Char => 1,
            Mark::Float => 1,
            Mark::Double => 1,
            Mark::Bytes(_) => 5,
            Mark::Str(_) => 5,
            Mark::Object(_) => 5,
            Mark::Enum(m) => 1 + m.mark_size(),
            Mark::Null => 1,
            Mark::Array(_, m) => 5 + m.mark_size(),
            Mark::List(_) => 5,
            Mark::Dict(_, k, v) => 5 + k.mark_size() + v.mark_size(),
            Mark::Map(_) => 5,
        }
    }

    /// Get the size in bytes that the data will use in binary form
    ///
    /// ```
    /// use mbon::data::Mark;
    ///
    /// assert_eq!(Mark::Int.data_size(), 4);
    /// ```
    pub fn data_size(&self) -> usize {
        match self {
            Mark::Long => 8,
            Mark::Int => 4,
            Mark::Short => 2,
            Mark::Char => 1,
            Mark::Float => 4,
            Mark::Double => 8,
            Mark::Bytes(n) => *n,
            Mark::Str(n) => *n,
            Mark::Object(n) => *n,
            Mark::Enum(m) => m.data_size() + 4,
            Mark::Null => 0,
            Mark::Array(len, m) => len * m.data_size(),
            Mark::List(n) => *n,
            Mark::Dict(len, k, v) => len * (k.data_size() + v.data_size()),
            Mark::Map(n) => *n,
        }
    }

    /// Get the total size in bytes that this value uses in binary form
    ///
    /// ```
    /// use mbon::data::Mark;
    ///
    /// assert_eq!(Mark::Int.size(), 5);
    /// ```
    #[inline]
    pub fn size(&self) -> usize {
        self.mark_size() + self.data_size()
    }

    /// Get the type of this mark
    pub fn get_type(&self) -> Type {
        match self {
            Mark::Long => Type::Long,
            Mark::Int => Type::Int,
            Mark::Short => Type::Short,
            Mark::Char => Type::Char,
            Mark::Float => Type::Float,
            Mark::Double => Type::Double,
            Mark::Bytes(_) => Type::Bytes,
            Mark::Str(_) => Type::Str,
            Mark::Object(_) => Type::Object,
            Mark::Enum(_) => Type::Enum,
            Mark::Null => Type::Null,
            Mark::Array(_, _) => Type::Array,
            Mark::List(_) => Type::List,
            Mark::Dict(_, _, _) => Type::Dict,
            Mark::Map(_) => Type::Map,
        }
    }

    /// Get the mark from a value
    ///
    /// ```
    /// use mbon::data::{Mark, Value};
    ///
    /// assert_eq!(Mark::from_value(Value::Int(32)), Mark::Int);
    /// assert_eq!(
    ///     Mark::from_value(Value::Str("Hello".to_owned())),
    ///     Mark::Str(5)
    /// );
    /// ```
    pub fn from_value(val: impl AsRef<Value>) -> Self {
        match val.as_ref() {
            Value::Long(_) => Self::Long,
            Value::Int(_) => Self::Int,
            Value::Short(_) => Self::Short,
            Value::Char(_) => Self::Char,
            Value::Float(_) => Self::Float,
            Value::Double(_) => Self::Double,
            Value::Bytes(v) => Self::Bytes(v.len()),
            Value::Str(v) => Self::Str(v.len()),
            Value::Object(v) => Self::Object(v.len()),
            Value::Enum(_, v) => Self::Enum(Box::new(Self::from_value(v))),
            Value::Null => Self::Null,
            Value::List(v) => {
                if Value::can_be_array(v) {
                    let first = v.first().unwrap();
                    Self::Array(v.len(), Box::new(Self::from_value(first)))
                } else {
                    Self::List(v.iter().map(|v| Self::from_value(v).size()).sum())
                }
            }
            Value::Map(v) => {
                if Value::can_be_dict(v) {
                    let (first_k, first_v) = v.first().unwrap();
                    Self::Dict(
                        v.len(),
                        Box::new(Self::from_value(first_k)),
                        Box::new(Self::from_value(first_v)),
                    )
                } else {
                    Self::Map(
                        v.iter()
                            .map(|(k, v)| Self::from_value(k).size() + Self::from_value(v).size())
                            .sum(),
                    )
                }
            }
        }
    }
}

impl AsRef<Mark> for Mark {
    fn as_ref(&self) -> &Mark {
        self
    }
}

impl From<Mark> for Type {
    fn from(m: Mark) -> Self {
        m.get_type()
    }
}

impl<'t> From<&'t Mark> for Type {
    fn from(m: &'t Mark) -> Self {
        m.get_type()
    }
}

impl From<Value> for Mark {
    fn from(v: Value) -> Self {
        Self::from_value(v)
    }
}

impl<'t> From<&'t Value> for Mark {
    fn from(v: &'t Value) -> Self {
        Self::from_value(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Long,
    Int,
    Short,
    Char,
    Float,
    Double,
    Bytes,
    Str,
    Object,
    Enum,
    Null,
    Array,
    List,
    Dict,
    Map,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Long => f.write_str("Long"),
            Type::Int => f.write_str("Int"),
            Type::Short => f.write_str("Short"),
            Type::Char => f.write_str("Char"),
            Type::Float => f.write_str("Float"),
            Type::Double => f.write_str("Double"),
            Type::Bytes => f.write_str("Bytes"),
            Type::Str => f.write_str("Str"),
            Type::Object => f.write_str("Object"),
            Type::Enum => f.write_str("Enum"),
            Type::Null => f.write_str("Null"),
            Type::Array => f.write_str("Array"),
            Type::List => f.write_str("List"),
            Type::Dict => f.write_str("Dict"),
            Type::Map => f.write_str("Map"),
        }
    }
}

impl Type {
    /// Get the prefix that will indicate the value type
    #[inline]
    pub fn prefix(&self) -> u8 {
        match self {
            Type::Long => b'l',
            Type::Int => b'i',
            Type::Short => b'h',
            Type::Char => b'c',
            Type::Float => b'f',
            Type::Double => b'd',
            Type::Bytes => b'b',
            Type::Str => b's',
            Type::Object => b'o',
            Type::Enum => b'e',
            Type::Null => b'n',
            Type::Array => b'a',
            Type::List => b'A',
            Type::Dict => b'm',
            Type::Map => b'M',
        }
    }

    /// Get the type from the given prefix
    pub fn from_prefix(prefix: u8) -> Result<Self, Error> {
        match prefix {
            b'l' => Ok(Type::Long),
            b'i' => Ok(Type::Int),
            b'h' => Ok(Type::Short),
            b'c' => Ok(Type::Char),
            b'f' => Ok(Type::Float),
            b'd' => Ok(Type::Double),
            b'b' => Ok(Type::Bytes),
            b's' => Ok(Type::Str),
            b'o' => Ok(Type::Object),
            b'e' => Ok(Type::Enum),
            b'n' => Ok(Type::Null),
            b'a' => Ok(Type::Array),
            b'A' => Ok(Type::List),
            b'm' => Ok(Type::Dict),
            b'M' => Ok(Type::Map),
            _ => Err(Error::DataError(format!("Unknown prefix `{}`", prefix))),
        }
    }
}
