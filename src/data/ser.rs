use crate::error::Error;

use super::Value;
use serde::ser::{self, Serializer};

pub struct ValueSer;
pub struct ValueListSer {
    list: Vec<Value>,
}
pub struct ValueMapSer {
    keys: Vec<Value>,
    values: Vec<Value>,
}
pub struct ValueEnumSer<T> {
    embed: T,
    variant: u32,
}

impl<'a> Serializer for &'a mut ValueSer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = ValueListSer;
    type SerializeTuple = ValueListSer;
    type SerializeTupleStruct = ValueListSer;
    type SerializeTupleVariant = ValueEnumSer<ValueListSer>;
    type SerializeMap = ValueMapSer;
    type SerializeStruct = ValueMapSer;
    type SerializeStructVariant = ValueEnumSer<ValueMapSer>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Char(v as i8))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Char(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Short(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Long(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Char(v as i8))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Short(v as i16))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(v as i32))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Long(v as i64))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Double(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        if v.is_ascii() {
            Ok(Value::Char(v as i8))
        } else {
            Ok(Value::Int(v as i32))
        }
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bytes(v.to_owned()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Enum(variant_index, Box::new(Value::Null)))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Ok(Value::Enum(
            variant_index,
            Box::new(value.serialize(&mut ValueSer)?),
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ValueListSer::new())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(ValueListSer::new())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(ValueListSer::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(ValueEnumSer::new(variant_index, ValueListSer::new()))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(ValueMapSer::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(ValueMapSer::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(ValueEnumSer::new(variant_index, ValueMapSer::new()))
    }
}

impl ValueListSer {
    fn new() -> Self {
        Self { list: Vec::new() }
    }

    fn add_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::Serialize,
    {
        self.list.push(value.serialize(&mut ValueSer)?);
        Ok(())
    }

    fn finish(self) -> Result<Value, Error> {
        Ok(Value::List(self.list))
    }
}

impl ValueMapSer {
    fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    fn add_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Error>
    where
        T: serde::Serialize,
    {
        self.keys.push(key.serialize(&mut ValueSer)?);
        Ok(())
    }

    fn add_val<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::Serialize,
    {
        self.values.push(value.serialize(&mut ValueSer)?);
        Ok(())
    }

    fn finish(self) -> Result<Value, Error> {
        let map = self.keys.into_iter().zip(self.values.into_iter()).collect();
        Ok(Value::Map(map))
    }
}

impl<T> ValueEnumSer<T> {
    fn new(variant: u32, embed: T) -> Self {
        Self { embed, variant }
    }
}

impl ser::SerializeSeq for ValueListSer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTuple for ValueListSer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTupleStruct for ValueListSer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeMap for ValueMapSer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_key(key)
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_val(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeStruct for ValueMapSer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.add_key(key)?;
        self.add_val(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl ser::SerializeTupleVariant for ValueEnumSer<ValueListSer> {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.embed.add_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let value = self.embed.finish()?;
        Ok(Value::Enum(self.variant, Box::new(value)))
    }
}

impl ser::SerializeStructVariant for ValueEnumSer<ValueMapSer> {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.embed.add_key(key)?;
        self.embed.add_val(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let value = self.embed.finish()?;
        Ok(Value::Enum(self.variant, Box::new(value)))
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::dumper::Dumper;

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
        let arr: Vec<u8> = vec![0, 1, 2, 3];

        let mut dumper = Dumper::new();
        dumper.write(&arr).unwrap();
        assert_eq!(dumper.buffer(), b"ac\x00\x00\x00\x04\x00\x01\x02\x03");
    }

    #[test]
    fn test_struct() {
        let data = Foo {
            a: 1,
            b: "Hello World".to_owned(),
            c: true,
        };

        let mut dumper = Dumper::new();
        dumper.write(&data).unwrap();
        assert_eq!(dumper.buffer(), b"M\x00\x00\x00\x29s\x00\x00\x00\x01ai\x00\x00\x00\x01s\x00\x00\x00\x01bs\x00\x00\x00\x0bHello Worlds\x00\x00\x00\x01cc\x01");
    }

    #[test]
    fn test_enum() {
        let foo = Bar::Foo;
        let cheese = Bar::Cheese(16);
        let hello = Bar::Hello { a: 16 };

        let expected = b"en\x00\x00\x00\x00ec\x00\x00\x00\x01\x10ems\x00\x00\x00\x01i\x00\x00\x00\x01\x00\x00\x00\x02a\x00\x00\x00\x10";

        let mut dumper = Dumper::new();
        dumper.write(&foo).unwrap();
        dumper.write(&cheese).unwrap();
        dumper.write(&hello).unwrap();
        assert_eq!(dumper.buffer(), expected);
    }
}
