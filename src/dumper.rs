//! # Dump mbon data
//!
//! Use [Dumper] to serialize mbon data.

use byteorder::{BigEndian, WriteBytesExt};
use serde::Serialize;

use std::io::Write;

use crate::{
    data::{ser::ValueSer, Mark, Type, Value},
    error::{Error, Result},
    object::ObjectDump,
};

/// A struct that writes binary data to a bytes buffer.
///
///
/// You can either write data that can be serialized using
/// * [`write()`](Dumper::write)
/// * [`write_obj()`](Dumper::write_obj)
///
/// Or you can write data directly using
/// * [`write_long()`](Dumper::write_long)
/// * [`write_int()`](Dumper::write_int)
/// * [`write_short()`](Dumper::write_short)
/// * [`write_char()`](Dumper::write_char)
/// * [`write_float()`](Dumper::write_float)
/// * [`write_double()`](Dumper::write_double)
/// * [`write_str()`](Dumper::write_str)
/// * [`write_bytes()`](Dumper::write_bytes)
/// * [`write_object()`](Dumper::write_object)
/// * [`write_enum()`](Dumper::write_enum)
/// * [`write_list()`](Dumper::write_list)
/// * [`write_map()`](Dumper::write_map)
#[derive(Debug)]
pub struct Dumper<W>(W);

impl<T> From<T> for Dumper<T>
where
    T: Write,
{
    fn from(t: T) -> Self {
        Dumper(t)
    }
}

impl<W> AsRef<W> for Dumper<W> {
    fn as_ref(&self) -> &W {
        &self.0
    }
}

impl<W> AsMut<W> for Dumper<W> {
    fn as_mut(&mut self) -> &mut W {
        &mut self.0
    }
}

impl Dumper<Vec<u8>> {
    #[inline]
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl<W> Dumper<W>
where
    W: Write,
{
    /// Get the underlying writer
    #[inline]
    pub fn writer(self) -> W {
        self.0
    }

    /// Get the underlying writer as a reference
    #[inline]
    pub fn get_writer(&self) -> &W {
        &self.0
    }

    /// Get the underlying writer as a mutable reference
    #[inline]
    pub fn get_writer_mut(&mut self) -> &mut W {
        &mut self.0
    }

    #[inline]
    fn write_data_long(&mut self, val: i64) -> Result<()> {
        self.0.write_i64::<BigEndian>(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_int(&mut self, val: i32) -> Result<()> {
        self.0.write_i32::<BigEndian>(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_short(&mut self, val: i16) -> Result<()> {
        self.0.write_i16::<BigEndian>(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_char(&mut self, val: i8) -> Result<()> {
        self.0.write_i8(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_float(&mut self, val: f32) -> Result<()> {
        self.0.write_f32::<BigEndian>(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_double(&mut self, val: f64) -> Result<()> {
        self.0.write_f64::<BigEndian>(val)?;
        Ok(())
    }

    #[inline]
    fn write_data_bytes(&mut self, val: &[u8]) -> Result<()> {
        Ok(self.0.write_all(val)?)
    }

    #[inline]
    fn write_data_str(&mut self, val: &str) -> Result<()> {
        Ok(self.0.write_all(val.as_bytes())?)
    }

    #[inline]
    fn write_data_enum(&mut self, variant: u32, val: impl AsRef<Value>) -> Result<()> {
        self.write_data_int(variant as i32)?;
        self.write_data_value(val)
    }

    fn write_data_array<'t, I>(&mut self, val: I) -> Result<()>
    where
        I: IntoIterator<Item = &'t Value>,
    {
        for v in val {
            self.write_data_value(v)?;
        }

        Ok(())
    }

    fn write_data_list<'t, I>(&mut self, val: I) -> Result<()>
    where
        I: IntoIterator<Item = &'t Value>,
    {
        for v in val {
            self.write_value(v)?;
        }
        Ok(())
    }

    fn write_data_dict<'t, I>(&mut self, val: I) -> Result<()>
    where
        I: IntoIterator<Item = &'t (Value, Value)>,
    {
        for (k, v) in val {
            self.write_data_value(k)?;
            self.write_data_value(v)?;
        }

        Ok(())
    }

    fn write_data_map<'t, I>(&mut self, val: I) -> Result<()>
    where
        I: IntoIterator<Item = &'t (Value, Value)>,
    {
        for (k, v) in val {
            self.write_value(k)?;
            self.write_value(v)?;
        }
        Ok(())
    }

    fn write_data_value(&mut self, val: impl AsRef<Value>) -> Result<()> {
        let val = val.as_ref();
        match val {
            Value::Long(v) => self.write_data_long(*v),
            Value::Int(v) => self.write_data_int(*v),
            Value::Short(v) => self.write_data_short(*v),
            Value::Char(v) => self.write_data_char(*v),
            Value::Float(v) => self.write_data_float(*v),
            Value::Double(v) => self.write_data_double(*v),
            Value::Bytes(v) => self.write_data_bytes(v),
            Value::Str(v) => self.write_data_str(v),
            Value::Object(v) => self.write_data_bytes(v),
            Value::Enum(var, v) => self.write_data_enum(*var, v),
            Value::Null => Ok(()),
            Value::List(v) => {
                if Value::can_be_array(v) {
                    self.write_data_array(v)
                } else {
                    self.write_data_list(v)
                }
            }
            Value::Map(v) => {
                if Value::can_be_dict(v) {
                    self.write_data_dict(v)
                } else {
                    self.write_data_map(v)
                }
            }
        }
    }

    #[inline]
    fn write_mark_long(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Long.prefix())?)
    }

    #[inline]
    fn write_mark_int(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Int.prefix())?)
    }

    #[inline]
    fn write_mark_short(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Short.prefix())?)
    }

    #[inline]
    fn write_mark_char(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Char.prefix())?)
    }

    #[inline]
    fn write_mark_float(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Float.prefix())?)
    }

    #[inline]
    fn write_mark_double(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Double.prefix())?)
    }

    #[inline]
    fn write_mark_null(&mut self) -> Result<()> {
        Ok(self.0.write_u8(Type::Null.prefix())?)
    }

    fn write_mark_bytes(&mut self, len: usize) -> Result<()> {
        self.0.write_u8(Type::Bytes.prefix())?;
        let len: u32 = len.try_into()?;
        self.write_data_int(len as i32)
    }

    fn write_mark_str(&mut self, len: usize) -> Result<()> {
        self.0.write_u8(Type::Str.prefix())?;
        let len: u32 = len.try_into()?;
        self.write_data_int(len as i32)
    }

    fn write_mark_object(&mut self, len: usize) -> Result<()> {
        self.0.write_u8(Type::Object.prefix())?;
        let len: u32 = len.try_into()?;
        self.write_data_int(len as i32)
    }

    fn write_mark_enum(&mut self, mark: impl AsRef<Mark>) -> Result<()> {
        self.0.write_u8(Type::Enum.prefix())?;
        self.write_mark(mark)
    }

    fn write_mark_array(&mut self, len: usize, mark: impl AsRef<Mark>) -> Result<()> {
        self.0.write_u8(Type::Array.prefix())?;
        self.write_mark(mark)?;
        let len: u32 = len.try_into()?;
        self.write_data_int(len as i32)
    }

    fn write_mark_list(&mut self, size: usize) -> Result<()> {
        self.0.write_u8(Type::List.prefix())?;
        let size: u32 = size.try_into()?;
        self.write_data_int(size as i32)
    }

    fn write_mark_dict(
        &mut self,
        len: usize,
        key_mark: impl AsRef<Mark>,
        val_mark: impl AsRef<Mark>,
    ) -> Result<()> {
        self.0.write_u8(Type::Dict.prefix())?;
        let len: u32 = len.try_into()?;
        self.write_mark(key_mark)?;
        self.write_mark(val_mark)?;
        self.write_data_int(len as i32)
    }

    fn write_mark_map(&mut self, size: usize) -> Result<()> {
        self.0.write_u8(Type::Map.prefix())?;
        let size: u32 = size.try_into()?;
        self.write_data_int(size as i32)
    }

    fn write_mark(&mut self, mark: impl AsRef<Mark>) -> Result<()> {
        match mark.as_ref() {
            Mark::Long => self.write_mark_long(),
            Mark::Int => self.write_mark_int(),
            Mark::Short => self.write_mark_short(),
            Mark::Char => self.write_mark_char(),
            Mark::Float => self.write_mark_float(),
            Mark::Double => self.write_mark_double(),
            Mark::Bytes(n) => self.write_mark_bytes(*n),
            Mark::Str(n) => self.write_mark_str(*n),
            Mark::Object(n) => self.write_mark_object(*n),
            Mark::Enum(m) => self.write_mark_enum(m),
            Mark::Null => self.write_mark_null(),
            Mark::Array(n, m) => self.write_mark_array(*n, m),
            Mark::List(s) => self.write_mark_list(*s),
            Mark::Dict(n, k, v) => self.write_mark_dict(*n, k, v),
            Mark::Map(s) => self.write_mark_map(*s),
        }
    }

    /// Write a serializeable object to the buffer.
    ///
    /// To use this function, your object must implement Serialize.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    /// use serde::Serialize;
    ///
    /// #[derive(Debug, Serialize)]
    /// struct Foo {
    ///     a: i32,
    ///     b: String,
    ///     c: f32,
    /// }
    ///
    /// let mut dumper = Dumper::new();
    /// let foo = Foo {
    ///     a: 42,
    ///     b: "Hello World".to_owned(),
    ///     c: 69.420
    /// };
    /// dumper.write(&foo).unwrap();
    ///
    /// ```
    pub fn write<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let value = value.serialize(&mut ValueSer)?;
        self.write_value(&value)
    }

    /// Write a binary object to the buffer.
    ///
    /// To use this function, your object must implement ObjectSerializer.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    /// use mbon::object::ObjectDump;
    /// use mbon::error::Error;
    ///
    /// struct Foo {
    ///     a: i32,
    ///     b: String,
    ///     c: f32,
    /// }
    ///
    /// impl ObjectDump for Foo {
    ///     type Error = Error;
    ///
    ///     fn dump_object(&self) -> Result<Vec<u8>, Self::Error> {
    ///         let mut dumper = Dumper::new();
    ///         dumper.write(&self.a)?;
    ///         dumper.write(&self.b)?;
    ///         dumper.write(&self.c)?;
    ///         Ok(dumper.writer())
    ///     }
    /// }
    ///
    /// let mut dumper = Dumper::new();
    /// let foo = Foo {
    ///     a: 42,
    ///     b: "Hello World".to_string(),
    ///     c: 69.420,
    /// };
    /// dumper.write_obj(&foo);
    /// ```
    pub fn write_obj<T>(&mut self, value: &T) -> Result<()>
    where
        T: ObjectDump,
        <T as ObjectDump>::Error: std::error::Error + 'static,
    {
        let data = Error::from_res(value.dump_object())?;
        self.write_object(&data)
    }

    /// Write a 64 bit integer to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_long(0x1020304050607080).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"l\x10\x20\x30\x40\x50\x60\x70\x80");
    /// ```
    pub fn write_long(&mut self, val: i64) -> Result<()> {
        self.write_mark_long()?;
        self.write_data_long(val)
    }

    /// Write a 32 bit integer to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_int(0x10203040).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"i\x10\x20\x30\x40");
    /// ```
    pub fn write_int(&mut self, val: i32) -> Result<()> {
        self.write_mark_int()?;
        self.write_data_int(val)
    }

    /// Write a 16 bit integer to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_short(0x1020).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"h\x10\x20");
    /// ```
    pub fn write_short(&mut self, val: i16) -> Result<()> {
        self.write_mark_short()?;
        self.write_data_short(val)
    }

    /// Write a 8 bit integer to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_char(0x10).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"c\x10");
    /// ```
    pub fn write_char(&mut self, val: i8) -> Result<()> {
        self.write_mark_char()?;
        self.write_data_char(val)
    }

    /// Write a 32 bit IEEE754 float to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_float(0.0).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"f\x00\x00\x00\x00");
    /// ```
    pub fn write_float(&mut self, val: f32) -> Result<()> {
        self.write_mark_float()?;
        self.write_data_float(val)
    }

    /// Write a 64 bit IEEE754 float to the dumper
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_double(0.0).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"d\x00\x00\x00\x00\x00\x00\x00\x00");
    /// ```
    pub fn write_double(&mut self, val: f64) -> Result<()> {
        self.write_mark_double()?;
        self.write_data_double(val)
    }

    /// Write a string of bytes to the dumper.
    ///
    /// Note: there can be at most 4294967295 bytes (4.29GB) in the bytearray.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_bytes(b"hello").unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"b\x00\x00\x00\x05hello");
    /// ```
    pub fn write_bytes(&mut self, val: impl AsRef<[u8]>) -> Result<()> {
        let val = val.as_ref();
        self.write_mark_bytes(val.len())?;
        self.write_data_bytes(val)
    }

    /// Write a string to the dumper.
    ///
    /// Note: there can be at most 4294967295 bytes (4.29GB) in the string.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_str("hello").unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"s\x00\x00\x00\x05hello");
    /// ```
    pub fn write_str(&mut self, val: impl AsRef<str>) -> Result<()> {
        let val = val.as_ref();
        self.write_mark_str(val.len())?;
        self.write_data_str(val)
    }

    /// Write a binary object to the dumper.
    ///
    /// This is meant for embedding binary data within the dumper.
    ///
    /// Note: there can be at most 4294967295 bytes (4.29GB) in the data.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_object("hello").unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"o\x00\x00\x00\x05hello");
    /// ```
    pub fn write_object(&mut self, val: impl AsRef<[u8]>) -> Result<()> {
        let val = val.as_ref();
        self.write_mark_object(val.len())?;
        self.write_data_bytes(val)
    }

    /// Write an indexed value to the dumper.
    ///
    /// This is meant for compatibility with rust enum serialization.
    ///
    /// An enum is stored as a variant and a value. The variant should determine
    /// the type of data that is stored.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    /// use mbon::data::Value;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_enum(1, Value::Int(0x3000)).unwrap();
    ///
    /// assert_eq!(dumper.writer(), b"ei\x00\x00\x00\x01\x00\x00\x30\x00");
    /// ```
    pub fn write_enum(&mut self, variant: u32, val: impl AsRef<Value>) -> Result<()> {
        let val = val.as_ref();
        self.write_mark_enum(Mark::from(val))?;
        self.write_data_enum(variant, val)
    }

    /// Write a null value to the dumper.
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_null();
    ///
    /// assert_eq!(dumper.writer(), b"n");
    /// ```
    pub fn write_null(&mut self) -> Result<()> {
        self.write_mark_null()
    }

    /// Write a list of values to the dumper.
    ///
    /// This can be written in two forms:
    /// * An array of fixed size items
    /// * A list of any type of item
    ///
    /// Note: when the list is stored as an array, there can be at most 4294967296
    /// items and when the list is stored as a list, the total size of the data
    /// can be no more than 4294967296 bytes (4.29 GB)
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    /// use mbon::data::Value;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_list(vec![
    ///     Value::Char(0x10),
    ///     Value::Char(0x20),
    ///     Value::Char(0x30),
    ///     Value::Char(0x40),
    ///     Value::Char(0x50)
    /// ]);
    ///
    /// assert_eq!(dumper.writer(), b"ac\x00\x00\x00\x05\x10\x20\x30\x40\x50");
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_list(vec![
    ///     Value::Char(0x10),
    ///     Value::Char(0x20),
    ///     Value::Char(0x30),
    ///     Value::Char(0x40),
    ///     Value::Str("Hello".to_owned())
    /// ]);
    ///
    /// assert_eq!(dumper.writer(),
    /// b"A\x00\x00\x00\x12c\x10c\x20c\x30c\x40s\x00\x00\x00\x05Hello");
    /// ```
    pub fn write_list(&mut self, val: impl AsRef<Vec<Value>>) -> Result<()> {
        let val = val.as_ref();
        if Value::can_be_array(val) {
            self.write_mark_array(val.len(), Mark::from(val.first().unwrap()))?;
            self.write_data_array(val)
        } else {
            self.write_mark_list(val.iter().map(|v| Mark::from(v).size()).sum())?;
            self.write_data_list(val)
        }
    }

    /// Write a key, value map of values to the dumper.
    ///
    /// This can be written in two forms:
    /// * An dict of fixed size key value pairs
    /// * A map of any type of key value pairs
    ///
    /// Note: when the map is stored as a dict, there can be at most 4294967296
    /// pairs and when the map is stored as a map, the total size of the data
    /// can be no more than 4294967296 bytes (4.29 GB)
    ///
    /// ```
    /// use mbon::dumper::Dumper;
    /// use mbon::data::Value;
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_map(vec![
    ///     (Value::Str("a".to_owned()), Value::Char(0x10)),
    ///     (Value::Str("b".to_owned()), Value::Char(0x20)),
    ///     (Value::Str("c".to_owned()), Value::Char(0x30)),
    /// ]);
    ///
    /// assert_eq!(dumper.writer(),
    /// b"ms\x00\x00\x00\x01c\x00\x00\x00\x03a\x10b\x20c\x30");
    ///
    /// let mut dumper = Dumper::new();
    /// dumper.write_map(vec![
    ///     (Value::Str("a".to_owned()), Value::Char(0x10)),
    ///     (Value::Str("b".to_owned()), Value::Char(0x20)),
    ///     (Value::Str("c".to_owned()), Value::Short(0x30)),
    /// ]);
    ///
    /// assert_eq!(dumper.writer(),
    /// b"M\x00\x00\x00\x19s\x00\x00\x00\x01ac\x10s\x00\x00\x00\x01bc\x20s\x00\x00\x00\x01ch\x00\x30");
    /// ```
    pub fn write_map(&mut self, val: impl AsRef<Vec<(Value, Value)>>) -> Result<()> {
        let val = val.as_ref();
        if Value::can_be_dict(val) {
            let (k, v) = val.first().unwrap();
            self.write_mark_dict(val.len(), Mark::from(k), Mark::from(v))?;
            self.write_data_dict(val)
        } else {
            self.write_mark_map(val.iter().map(|(k, v)| k.size() + v.size()).sum())?;
            self.write_data_map(val)
        }
    }

    /// Write any value to the dumper.
    ///
    /// This will call the appropriate function for the given value type.
    pub fn write_value(&mut self, val: impl AsRef<Value>) -> Result<()> {
        let val = val.as_ref();
        match val {
            Value::Long(v) => self.write_long(*v),
            Value::Int(v) => self.write_int(*v),
            Value::Short(v) => self.write_short(*v),
            Value::Char(v) => self.write_char(*v),
            Value::Float(v) => self.write_float(*v),
            Value::Double(v) => self.write_double(*v),
            Value::Bytes(v) => self.write_bytes(v),
            Value::Str(v) => self.write_str(v),
            Value::Object(v) => self.write_object(v),
            Value::Enum(variant, v) => self.write_enum(*variant, v),
            Value::Null => self.write_null(),
            Value::List(v) => self.write_list(v),
            Value::Map(v) => self.write_map(v),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_long() {
        let mut dumper = Dumper::new();
        dumper.write_long(0x3040).unwrap();
        assert_eq!(dumper.0, b"l\x00\x00\x00\x00\x00\x00\x30\x40");
    }

    #[test]
    fn test_int() {
        let mut dumper = Dumper::new();
        dumper.write_int(0x3040).unwrap();
        assert_eq!(dumper.0, b"i\x00\x00\x30\x40");
    }

    #[test]
    fn test_short() {
        let mut dumper = Dumper::new();
        dumper.write_short(0x3040).unwrap();
        assert_eq!(dumper.0, b"h\x30\x40");
    }

    #[test]
    fn test_char() {
        let mut dumper = Dumper::new();
        dumper.write_char(0x40).unwrap();
        assert_eq!(dumper.0, b"c\x40");
    }

    #[test]
    fn test_float() {
        let mut dumper = Dumper::new();
        dumper.write_float(0.0).unwrap();
        assert_eq!(dumper.0, b"f\x00\x00\x00\x00");
    }

    #[test]
    fn test_double() {
        let mut dumper = Dumper::new();
        dumper.write_double(0.0).unwrap();
        assert_eq!(dumper.0, b"d\x00\x00\x00\x00\x00\x00\x00\x00");
    }

    #[test]
    fn test_bytes() {
        let mut dumper = Dumper::new();
        dumper.write_bytes(b"Hello world!").unwrap();
        assert_eq!(dumper.0, b"b\x00\x00\x00\x0cHello world!");
    }

    #[test]
    fn test_str() {
        let mut dumper = Dumper::new();
        dumper.write_str("Hello world!").unwrap();
        assert_eq!(dumper.0, b"s\x00\x00\x00\x0cHello world!");
    }

    #[test]
    fn test_object() {
        let mut dumper = Dumper::new();
        dumper.write_object(b"Hello world!").unwrap();
        assert_eq!(dumper.0, b"o\x00\x00\x00\x0cHello world!");
    }

    #[test]
    fn test_enum() {
        let mut dumper = Dumper::new();
        dumper.write_enum(1, &Value::Int(4)).unwrap();
        assert_eq!(dumper.0, b"ei\x00\x00\x00\x01\x00\x00\x00\x04");
    }

    #[test]
    fn test_null() {
        let mut dumper = Dumper::new();
        dumper.write_null().unwrap();
        assert_eq!(dumper.0, b"n");
    }

    #[test]
    fn test_array() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::Char(1),
            Value::Char(2),
            Value::Char(3),
            Value::Char(4),
            Value::Char(5),
        ];
        dumper.write_list(&value).unwrap();
        assert_eq!(dumper.0, b"ac\x00\x00\x00\x05\x01\x02\x03\x04\x05");
    }

    #[test]
    fn test_obj_array() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::Object(b"i\x00\x00\x00\x69c\x01".to_vec()),
            Value::Object(b"i\x00\x00\x00\x10c\x02".to_vec()),
            Value::Object(b"i\x00\x00\x00\x42c\x03".to_vec()),
        ];
        dumper.write_list(&value).unwrap();
        assert_eq!(dumper.0, b"ao\x00\x00\x00\x07\x00\x00\x00\x03i\x00\x00\x00\x69c\x01i\x00\x00\x00\x10c\x02i\x00\x00\x00\x42c\x03")
    }

    #[test]
    fn test_obj_array_bad_size() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::Object(b"i\x00\x00\x00\x69c\x01".to_vec()),
            Value::Object(b"i\x00\x00\x00\x10h\x00\x02".to_vec()),
            Value::Object(b"i\x00\x00\x00\x42c\x03".to_vec()),
        ];
        dumper.write_list(&value).unwrap();
        assert_eq!(dumper.0, b"A\x00\x00\x00\x25o\x00\x00\x00\x07i\x00\x00\x00\x69c\x01o\x00\x00\x00\x08i\x00\x00\x00\x10h\x00\x02o\x00\x00\x00\x07i\x00\x00\x00\x42c\x03")
    }

    #[test]
    fn test_obj_array_bad_type() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::Object(b"i\x00\x00\x00\x69c\x01".to_vec()),
            Value::Bytes(b"i\x00\x00\x00\x10c\x02".to_vec()),
            Value::Object(b"i\x00\x00\x00\x42c\x03".to_vec()),
        ];
        dumper.write_list(&value).unwrap();
        assert_eq!(dumper.0, b"A\x00\x00\x00\x24o\x00\x00\x00\x07i\x00\x00\x00\x69c\x01b\x00\x00\x00\x07i\x00\x00\x00\x10c\x02o\x00\x00\x00\x07i\x00\x00\x00\x42c\x03")
    }

    #[test]
    fn test_2d_array() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::List(vec![
                Value::Char(1),
                Value::Char(2),
                Value::Char(3),
                Value::Char(4),
                Value::Char(5),
            ]),
            Value::List(vec![
                Value::Char(6),
                Value::Char(7),
                Value::Char(8),
                Value::Char(9),
                Value::Char(10),
            ]),
            Value::List(vec![
                Value::Char(11),
                Value::Char(12),
                Value::Char(13),
                Value::Char(14),
                Value::Char(15),
            ]),
        ];

        dumper.write_list(value).unwrap();

        assert_eq!(
            dumper.0,
            b"aac\x00\x00\x00\x05\x00\x00\x00\x03\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f"
        )
    }

    #[test]
    fn test_list() {
        let mut dumper = Dumper::new();
        let value = vec![
            Value::Str("Hello".into()),
            Value::Char(2),
            Value::Char(3),
            Value::Char(4),
            Value::Char(5),
        ];
        dumper.write_list(&value).unwrap();
        assert_eq!(
            dumper.0,
            b"A\x00\x00\x00\x12s\x00\x00\x00\x05Helloc\x02c\x03c\x04c\x05"
        );
    }

    #[test]
    fn test_map() {
        let mut dumper = Dumper::new();
        let value = vec![
            (Value::Str("a".into()), Value::Char(2)),
            (Value::Str("b".into()), Value::Short(5)),
        ];
        dumper.write_map(&value).unwrap();
        assert_eq!(
            dumper.0,
            b"M\x00\x00\x00\x11s\x00\x00\x00\x01ac\x02s\x00\x00\x00\x01bh\x00\x05"
        );
    }

    #[test]
    fn test_dict() {
        let mut dumper = Dumper::new();
        let value = vec![
            (Value::Str("a".into()), Value::Char(2)),
            (Value::Str("b".into()), Value::Char(5)),
        ];
        dumper.write_map(&value).unwrap();
        assert_eq!(dumper.0, b"ms\x00\x00\x00\x01c\x00\x00\x00\x02a\x02b\x05");
    }
}
