//! # Serde Deserializer implementation for [Value]
//!
//! [Value]: mbon::data::Value

use crate::error::Error;

use super::{Type, Value};
use serde::{de, Deserializer};

pub struct ValueDe<'de> {
    input: &'de Value,
}
pub struct ValueSeqAccess<'de> {
    seq: &'de Vec<Value>,
    index: usize,
}
pub struct ValueMapAccess<'de> {
    seq: &'de Vec<(Value, Value)>,
    index: usize,
}
pub struct ValueEnumAccess<'de> {
    parent: &'de Value,
    value: &'de Value,
}

impl<'de> ValueDe<'de> {
    #[inline]
    pub fn new(input: &'de Value) -> Self {
        Self { input }
    }
}

impl<'de> ValueSeqAccess<'de> {
    #[inline]
    fn new(seq: &'de Vec<Value>) -> Self {
        Self { seq, index: 0 }
    }
}

impl<'de> ValueMapAccess<'de> {
    #[inline]
    fn new(seq: &'de Vec<(Value, Value)>) -> Self {
        Self { seq, index: 0 }
    }
}

impl<'de> ValueEnumAccess<'de> {
    #[inline]
    fn new(parent: &'de Value, value: &'de Value) -> Self {
        Self { parent, value }
    }
}

impl<'de> ValueDe<'de> {
    fn next_i8(&self) -> Result<i8, Error> {
        match self.input {
            Value::Long(v) => Ok(i8::try_from(*v)?),
            Value::Int(v) => Ok(i8::try_from(*v)?),
            Value::Short(v) => Ok(i8::try_from(*v)?),
            Value::Char(v) => Ok(*v),
            _ => Err(Error::Expected(Type::Char)),
        }
    }

    fn next_i16(&self) -> Result<i16, Error> {
        match self.input {
            Value::Long(v) => Ok(i16::try_from(*v)?),
            Value::Int(v) => Ok(i16::try_from(*v)?),
            Value::Short(v) => Ok(*v),
            Value::Char(v) => Ok(*v as i16),
            _ => Err(Error::Expected(Type::Short)),
        }
    }

    fn next_i32(&self) -> Result<i32, Error> {
        match self.input {
            Value::Long(v) => Ok(i32::try_from(*v)?),
            Value::Int(v) => Ok(*v),
            Value::Short(v) => Ok(*v as i32),
            Value::Char(v) => Ok(*v as i32),
            _ => Err(Error::Expected(Type::Int)),
        }
    }

    fn next_i64(&self) -> Result<i64, Error> {
        match self.input {
            Value::Long(v) => Ok(*v),
            Value::Int(v) => Ok(*v as i64),
            Value::Short(v) => Ok(*v as i64),
            Value::Char(v) => Ok(*v as i64),
            _ => Err(Error::Expected(Type::Long)),
        }
    }

    fn next_u8(&self) -> Result<u8, Error> {
        match self.input {
            Value::Long(v) => Ok(u8::try_from(*v as u64)?),
            Value::Int(v) => Ok(u8::try_from(*v as u32)?),
            Value::Short(v) => Ok(u8::try_from(*v as u16)?),
            Value::Char(v) => Ok(*v as u8),
            _ => Err(Error::Expected(Type::Char)),
        }
    }

    fn next_u16(&self) -> Result<u16, Error> {
        match self.input {
            Value::Long(v) => Ok(u16::try_from(*v as u64)?),
            Value::Int(v) => Ok(u16::try_from(*v as u32)?),
            Value::Short(v) => Ok(*v as u16),
            Value::Char(v) => Ok(*v as u16),
            _ => Err(Error::Expected(Type::Short)),
        }
    }

    fn next_u32(&self) -> Result<u32, Error> {
        match self.input {
            Value::Long(v) => Ok(u32::try_from(*v as u64)?),
            Value::Int(v) => Ok(*v as u32),
            Value::Short(v) => Ok((*v as u16) as u32),
            Value::Char(v) => Ok((*v as u8) as u32),
            _ => Err(Error::Expected(Type::Int)),
        }
    }

    fn next_u64(&self) -> Result<u64, Error> {
        match self.input {
            Value::Long(v) => Ok(*v as u64),
            Value::Int(v) => Ok((*v as u32) as u64),
            Value::Short(v) => Ok((*v as u16) as u64),
            Value::Char(v) => Ok((*v as u8) as u64),
            _ => Err(Error::Expected(Type::Long)),
        }
    }

    fn next_f32(&self) -> Result<f32, Error> {
        match self.input {
            Value::Double(v) => Ok(*v as f32),
            Value::Float(v) => Ok(*v),
            _ => Err(Error::Expected(Type::Float)),
        }
    }

    fn next_f64(&self) -> Result<f64, Error> {
        match self.input {
            Value::Double(v) => Ok(*v),
            Value::Float(v) => Ok(*v as f64),
            _ => Err(Error::Expected(Type::Float)),
        }
    }

    fn next_bytes(&self) -> Result<&Vec<u8>, Error> {
        match self.input {
            Value::Bytes(v) => Ok(v),
            Value::Object(v) => Ok(v),
            _ => Err(Error::Expected(Type::Bytes)),
        }
    }

    fn next_str(&self) -> Result<&str, Error> {
        match self.input {
            Value::Str(v) => Ok(v),
            _ => Err(Error::Expected(Type::Str)),
        }
    }
}

impl<'de> de::Deserializer<'de> for ValueDe<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Value::Long(v) => visitor.visit_i64(*v),
            Value::Int(v) => visitor.visit_i32(*v),
            Value::Short(v) => visitor.visit_i16(*v),
            Value::Char(v) => visitor.visit_i8(*v),
            Value::Float(v) => visitor.visit_f32(*v),
            Value::Double(v) => visitor.visit_f64(*v),
            Value::Bytes(v) => visitor.visit_bytes(v),
            Value::Str(v) => visitor.visit_str(v),
            Value::Object(v) => visitor.visit_bytes(v),
            Value::Enum(_, v) => visitor.visit_enum(ValueEnumAccess::new(self.input, v)),
            Value::Null => visitor.visit_unit(),
            Value::List(v) => visitor.visit_seq(ValueSeqAccess::new(v)),
            Value::Map(v) => visitor.visit_map(ValueMapAccess::new(v)),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_bool(self.next_i64()? != 0)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_i8(self.next_i8()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_i16(self.next_i16()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_i32(self.next_i32()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_i64(self.next_i64()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_u8(self.next_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_u16(self.next_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_u32(self.next_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_u64(self.next_u64()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_f32(self.next_f32()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_f64(self.next_f64()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let c = match self.input {
            Value::Char(v) => (*v as u8) as char,
            _ => char::from_u32(self.next_u32()?)
                .ok_or(Error::data_error("Invalid UTF-8 Character"))?,
        };
        visitor.visit_char(c)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_str(self.next_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.next_str()?.to_owned())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_bytes(self.next_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_byte_buf(self.next_bytes()?.to_owned())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if let Value::Null = self.input {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if let Value::Null = self.input {
            visitor.visit_unit()
        } else {
            Err(Error::Expected(Type::Null))
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if let Value::List(v) = self.input {
            visitor.visit_seq(ValueSeqAccess::new(v))
        } else {
            Err(Error::Expected(Type::List))
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if let Value::Map(v) = self.input {
            visitor.visit_map(ValueMapAccess::new(v))
        } else {
            Err(Error::Expected(Type::Map))
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if let Value::Enum(_, v) = self.input {
            visitor.visit_enum(ValueEnumAccess::new(self.input, v))
        } else {
            Err(Error::Expected(Type::Enum))
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.input {
            Value::Str(v) => visitor.visit_str(v),
            Value::Enum(variant, _) => visitor.visit_u32(*variant),
            _ => Err(Error::Expected(Type::Str)),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

impl<'de> de::SeqAccess<'de> for ValueSeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        Ok(if let Some(value) = self.seq.get(self.index) {
            self.index += 1;
            Some(seed.deserialize(ValueDe::new(value))?)
        } else {
            None
        })
    }
}

impl<'de> de::MapAccess<'de> for ValueMapAccess<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        Ok(if let Some((key, _value)) = self.seq.get(self.index) {
            Some(seed.deserialize(ValueDe::new(key))?)
        } else {
            None
        })
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        if let Some((_key, value)) = self.seq.get(self.index) {
            self.index += 1;
            Ok(seed.deserialize(ValueDe::new(value))?)
        } else {
            Err(Error::Msg("Expected index to be in bounds".into()))
        }
    }
}

impl<'de> de::EnumAccess<'de> for ValueEnumAccess<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(ValueDe::new(self.parent))?;
        Ok((variant, self))
    }
}

impl<'de> de::VariantAccess<'de> for ValueEnumAccess<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(ValueDe::new(self.value))
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let de = ValueDe::new(self.value);
        de.deserialize_seq(visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let de = ValueDe::new(self.value);
        de.deserialize_map(visitor)
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::{error::Error, parser::Parser};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Foo {
        a: i32,
        b: String,
        c: bool,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    enum Bar {
        Foo,
        Cheese(i8),
        Hello { a: i32 },
    }

    #[test]
    fn test_vec() {
        let mut parser = Parser::from(b"ac\x00\x00\x00\x04\x00\x01\x02\x03");
        let arr: Vec<u8> = parser.next().unwrap();
        assert_eq!(arr, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_struct() {
        let mut parser = Parser::from(b"M\x00\x00\x00\x29s\x00\x00\x00\x01ai\x00\x00\x00\x01s\x00\x00\x00\x01bs\x00\x00\x00\x0bHello Worlds\x00\x00\x00\x01cc\x01");
        let arr: Foo = parser.next().unwrap();
        assert_eq!(
            arr,
            Foo {
                a: 1,
                b: "Hello World".to_owned(),
                c: true
            }
        );
    }

    #[test]
    fn test_enum() {
        let data = b"en\x00\x00\x00\x00ec\x00\x00\x00\x01\x10eM\x00\x00\x00\x0b\x00\x00\x00\x02s\x00\x00\x00\x01ai\x00\x00\x00\x10";

        let mut parser = Parser::from(data);

        let foo: Bar = parser.next().unwrap();
        assert_eq!(foo, Bar::Foo);

        let cheese: Bar = parser.next().unwrap();
        assert_eq!(cheese, Bar::Cheese(16));

        let hello: Bar = parser.next().unwrap();
        assert_eq!(hello, Bar::Hello { a: 16 });
    }

    #[test]
    fn test_expected() {
        let mut parser = Parser::from(b"s\x00\x00\x00\x02hi");

        let err = parser.next::<i32>().expect_err("Error::Expected");
        if let Error::Expected(_) = err {
        } else {
            panic!("Expected Error::Expected");
        }
    }

    #[test]
    fn test_int_coersion() {
        let mut parser = Parser::from(b"c\x32");

        let val: i32 = parser.next().unwrap();
        assert_eq!(val, 0x32);
    }

    #[test]
    fn test_bad_int_coersion() {
        let mut parser = Parser::from(b"i\x40\x00\x00\x00");

        let err = parser.next::<i16>().expect_err("TryFromIntError");
        if let Error::DataError(_) = err {
        } else {
            panic!("Expected TryFromIntError");
        }
    }

    #[test]
    fn test_big_int_coersion() {
        let mut parser = Parser::from(b"i\x00\x00\x00\x40");

        let val: u8 = parser.next().unwrap();
        assert_eq!(val, 0x40);
    }
}
