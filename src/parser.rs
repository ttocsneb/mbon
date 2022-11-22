//! # Parse mbon data
//!
//! Use [Parser] to deserialize mbon data.

use crate::{
    data::{Mark, Type, Value},
    error::Error,
    object::ObjectParse,
};
use byteorder::{BigEndian, ReadBytesExt};
use serde::de::DeserializeOwned;

use std::io::{Read, Seek, SeekFrom};

/// A struct that parses binary data from a bytearray
///
/// You can deserialize data using
/// * [`next()`](Parser::next)
/// * [`next_obj()`](Parser::next_obj)
///
/// Or you can deserialize data directly using
/// * [`next_value()`](Parser::next_value)
pub struct Parser<R>(R);

impl<'a, T> From<&'a T> for Parser<&'a [u8]>
where
    T: AsRef<[u8]>,
{
    fn from(slice: &'a T) -> Self {
        Self::new(slice.as_ref())
    }
}

impl<R> Parser<R>
where
    R: Read,
{
    /// Create a new Parser from a reader
    #[inline]
    pub fn new(data: R) -> Self {
        Self(data)
    }

    /// Parse the next item in the parser.
    ///
    /// ```
    /// use mbon::parser::Parser;
    ///
    /// let mut parser = Parser::from(b"i\x00\x00\x00\x42");
    /// let i: u32 = parser.next().unwrap();
    ///
    /// assert_eq!(i, 0x42);
    /// ```
    #[inline]
    pub fn next<T>(&mut self) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        self.next_value()?.parse()
    }

    /// Parse the next custom object in the parser.
    ///
    /// This allows you to be able to parse custom binary data. A common usecase
    /// is to store a struct in a more compact form. You could also use object
    /// values to store a different format altogether.
    ///
    /// Note: the next value in the parser must be an Object
    ///
    /// ```
    /// use mbon::error::Error;
    /// use mbon::parser::Parser;
    /// use mbon::object::ObjectParse;
    ///
    /// struct Foo {
    ///     a: i32,
    ///     b: String,
    ///     c: f32,
    /// }
    ///
    /// impl ObjectParse for Foo {
    ///     type Error = Error;
    ///
    ///     fn parse_object(data: &[u8]) -> Result<Self, Self::Error> {
    ///         let mut parser = Parser::new(data);
    ///         let a = parser.next()?;
    ///         let b = parser.next()?;
    ///         let c = parser.next()?;
    ///         Ok(Self { a, b, c })
    ///     }
    /// }
    ///
    /// let mut parser =
    /// Parser::from(
    ///     b"o\x00\x00\x00\x14i\x00\x00\x00\x42s\x00\x00\x00\x05Hellof\x00\x00\x00\x00"
    /// );
    ///
    /// let foo: Foo = parser.next_obj().unwrap();
    /// assert_eq!(foo.a, 0x42);
    /// assert_eq!(foo.b, "Hello");
    /// assert_eq!(foo.c, 0.0);
    /// ```
    #[inline]
    pub fn next_obj<T>(&mut self) -> Result<T, Error>
    where
        T: ObjectParse,
        <T as ObjectParse>::Error: std::error::Error + 'static,
    {
        self.next_value()?.parse_obj()
    }

    #[inline]
    fn next_type(&mut self) -> Result<Type, Error> {
        Type::from_prefix(self.0.read_u8()?)
    }

    fn next_data_n(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; n];
        self.0.read_exact(&mut buf)?;
        Ok(buf)
    }

    #[inline]
    fn next_data_long(&mut self) -> Result<i64, Error> {
        Ok(self.0.read_i64::<BigEndian>()?)
    }

    #[inline]
    fn next_data_int(&mut self) -> Result<i32, Error> {
        Ok(self.0.read_i32::<BigEndian>()?)
    }

    #[inline]
    fn next_data_short(&mut self) -> Result<i16, Error> {
        Ok(self.0.read_i16::<BigEndian>()?)
    }

    #[inline]
    fn next_data_char(&mut self) -> Result<i8, Error> {
        Ok(self.0.read_i8()?)
    }

    #[inline]
    fn next_data_float(&mut self) -> Result<f32, Error> {
        Ok(self.0.read_f32::<BigEndian>()?)
    }

    #[inline]
    fn next_data_double(&mut self) -> Result<f64, Error> {
        Ok(self.0.read_f64::<BigEndian>()?)
    }

    #[inline]
    fn next_data_bytes(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        self.next_data_n(n)
    }

    #[inline]
    fn next_data_str(&mut self, n: usize) -> Result<String, Error> {
        let buf = self.next_data_n(n)?;
        Ok(String::from_utf8(buf)?)
    }

    fn next_data_enum(&mut self, m: &Mark) -> Result<(u32, Value), Error> {
        let variant = self.next_data_int()? as u32;
        let value = self.next_data_value(m)?;
        Ok((variant, value))
    }

    fn next_data_array(&mut self, len: usize, t: &Mark) -> Result<Vec<Value>, Error> {
        let mut arr = Vec::with_capacity(len);

        for _ in 0..len {
            let v = self.next_data_value(t)?;
            arr.push(v);
        }

        Ok(arr)
    }

    fn next_data_list(&mut self, size: usize) -> Result<Vec<Value>, Error> {
        let mut arr = Vec::new();

        let mut read = 0;

        while read < size {
            let m = self.next_mark()?;
            let v = self.next_data_value(&m)?;
            arr.push(v);
            read += m.size();
        }

        if read > size {
            return Err(Error::data_error("List was larger than expected"));
        }

        Ok(arr)
    }

    fn next_data_dict(
        &mut self,
        len: usize,
        k: &Mark,
        v: &Mark,
    ) -> Result<Vec<(Value, Value)>, Error> {
        let mut arr = Vec::with_capacity(len);

        for _ in 0..len {
            let key = self.next_data_value(k)?;
            let val = self.next_data_value(v)?;
            arr.push((key, val));
        }

        Ok(arr)
    }

    fn next_data_map(&mut self, size: usize) -> Result<Vec<(Value, Value)>, Error> {
        let mut arr = Vec::new();
        let mut read = 0;

        while read < size {
            let k = self.next_mark()?;
            let key = self.next_data_value(&k)?;
            let v = self.next_mark()?;
            let val = self.next_data_value(&v)?;

            arr.push((key, val));
            read += k.size() + v.size();
        }

        if read > size {
            return Err(Error::data_error("Map was larger than expected"));
        }

        Ok(arr)
    }

    fn next_data_value(&mut self, mark: &Mark) -> Result<Value, Error> {
        Ok(match mark {
            Mark::Long => Value::Long(self.next_data_long()?),
            Mark::Int => Value::Int(self.next_data_int()?),
            Mark::Short => Value::Short(self.next_data_short()?),
            Mark::Char => Value::Char(self.next_data_char()?),
            Mark::Float => Value::Float(self.next_data_float()?),
            Mark::Double => Value::Double(self.next_data_double()?),
            Mark::Bytes(n) => Value::Bytes(self.next_data_bytes(*n)?),
            Mark::Str(n) => Value::Str(self.next_data_str(*n)?.to_owned()),
            Mark::Object(n) => Value::Object(self.next_data_bytes(*n)?.to_vec()),
            Mark::Enum(m) => {
                let (var, val) = self.next_data_enum(&m)?;
                Value::Enum(var, Box::new(val))
            }
            Mark::Null => Value::Null,
            Mark::Array(n, m) => Value::List(self.next_data_array(*n, &m)?),
            Mark::List(n) => Value::List(self.next_data_list(*n)?),
            Mark::Dict(n, k, v) => Value::Map(self.next_data_dict(*n, &k, &v)?),
            Mark::Map(n) => Value::Map(self.next_data_map(*n)?),
        })
    }

    fn next_mark(&mut self) -> Result<Mark, Error> {
        let t = self.next_type()?;
        Ok(match t {
            Type::Long => Mark::Long,
            Type::Int => Mark::Int,
            Type::Short => Mark::Short,
            Type::Char => Mark::Char,
            Type::Float => Mark::Float,
            Type::Double => Mark::Double,
            Type::Bytes => Mark::Bytes(self.next_data_int()? as usize),
            Type::Str => Mark::Str(self.next_data_int()? as usize),
            Type::Object => Mark::Object(self.next_data_int()? as usize),
            Type::Enum => Mark::Enum(Box::new(self.next_mark()?)),
            Type::Null => Mark::Null,
            Type::Array => {
                let mark = self.next_mark()?;
                let len = self.next_data_int()? as usize;
                Mark::Array(len, Box::new(mark))
            }
            Type::List => Mark::List(self.next_data_int()? as usize),
            Type::Dict => {
                let k = self.next_mark()?;
                let v = self.next_mark()?;
                let len = self.next_data_int()? as usize;
                Mark::Dict(len, Box::new(k), Box::new(v))
            }
            Type::Map => Mark::Map(self.next_data_int()? as usize),
        })
    }

    /// Skip the next value in the parser.
    ///
    /// This will ignore the next value without parsing more than what's
    /// necessary.
    ///
    /// If the reader supports seeking, then it is preffered to use
    /// [`seek_next()`](Parser::seek_next) instead.
    ///
    /// ```
    /// use mbon::parser::Parser;
    ///
    /// let mut parser = Parser::from(
    ///     b"s\x00\x00\x00\x1eI don't care about this stringi\x00\x00\x00\x42"
    /// );
    ///
    /// parser.skip_next().unwrap();
    ///
    /// let v: i32 = parser.next().unwrap();
    /// assert_eq!(v, 0x42);
    /// ```
    pub fn skip_next(&mut self) -> Result<(), Error> {
        let mark = self.next_mark()?;
        let size = mark.data_size();

        self.next_data_n(size)?;

        Ok(())
    }

    /// Parse the next value in the parser.
    ///
    /// This will try to read whatever value is next and return it.
    ///
    /// ```
    /// use mbon::parser::Parser;
    /// use mbon::data::Value;
    ///
    /// let mut parser = Parser::from(b"i\x00\x00\x00\x42");
    ///
    /// assert_eq!(parser.next_value().unwrap(), Value::Int(0x42));
    /// ```
    #[inline]
    pub fn next_value(&mut self) -> Result<Value, Error> {
        let mark = self.next_mark()?;
        self.next_data_value(&mark)
    }
}

impl<R> Parser<R>
where
    R: Read + Seek,
{
    /// Seek to the next value in the parser.
    ///
    /// This will efficiently skip the next value without reading more than
    /// what's necessary.
    ///
    pub fn seek_next(&mut self) -> Result<(), Error> {
        let mark = self.next_mark()?;
        let size = mark.data_size();

        self.0.seek(SeekFrom::Current(size as i64))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_long() {
        let mut parser = Parser::from(b"l\x00\x30\x00\x00\x20\x10\x00\x05");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Long(0x0030000020100005));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_int() {
        let mut parser = Parser::from(b"i\x03\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Int(0x03000000));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_short() {
        let mut parser = Parser::from(b"h\x03\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Short(0x0300));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_char() {
        let mut parser = Parser::from(b"c\x03");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Char(0x03));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_float() {
        let mut parser = Parser::from(b"f\x00\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Float(0.0));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_double() {
        let mut parser = Parser::from(b"d\x00\x00\x00\x00\x00\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Double(0.0));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_bytes() {
        let mut parser = Parser::from(b"b\x00\x00\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Bytes(b"Hello World".to_vec()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_str() {
        let mut parser = Parser::from(b"s\x00\x00\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Str("Hello World".to_owned()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_object() {
        let mut parser = Parser::from(b"o\x00\x00\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Object(b"Hello World".to_vec()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_enum() {
        let mut parser = Parser::from(b"ei\x00\x00\x00\x01\x00\x00\x00\xfe");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Enum(1, Box::new(Value::Int(0xfe))));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_null() {
        let mut parser = Parser::from(b"n");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Null);
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_array() {
        let mut parser = Parser::from(b"ac\x00\x00\x00\x04\x01\x02\x03\x04");
        let val = parser.next_value().unwrap();
        if let Value::List(val) = val {
            assert_eq!(val.len(), 4);
            assert_eq!(val.get(0).unwrap().to_owned(), Value::Char(1));
            assert_eq!(val.get(1).unwrap().to_owned(), Value::Char(2));
            assert_eq!(val.get(2).unwrap().to_owned(), Value::Char(3));
            assert_eq!(val.get(3).unwrap().to_owned(), Value::Char(4));
        } else {
            panic!("value is not a list");
        }
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_2d_array() {
        let mut parser = Parser::from(
            b"aac\x00\x00\x00\x05\x00\x00\x00\x03\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f",
        );
        let val = parser.next_value().unwrap();
        if let Value::List(val) = val {
            let mut i = 1;
            for v in val {
                if let Value::List(v) = v {
                    for item in v {
                        assert_eq!(item, Value::Char(i));
                        i += 1;
                    }
                } else {
                    panic!("nested value is not a list");
                }
            }
        } else {
            panic!("value is not a list")
        }
    }

    #[test]
    fn test_list() {
        let mut parser = Parser::from(b"A\x00\x00\x00\x08c\x01c\x02c\x03c\x04");
        let val = parser.next_value().unwrap();
        if let Value::List(val) = val {
            assert_eq!(val.len(), 4);
            assert_eq!(val.get(0).unwrap().to_owned(), Value::Char(1));
            assert_eq!(val.get(1).unwrap().to_owned(), Value::Char(2));
            assert_eq!(val.get(2).unwrap().to_owned(), Value::Char(3));
            assert_eq!(val.get(3).unwrap().to_owned(), Value::Char(4));
        } else {
            panic!("value is not a list");
        }
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_map() {
        let mut parser =
            Parser::from(b"M\x00\x00\x00\x10s\x00\x00\x00\x01ac\x01s\x00\x00\x00\x01bc\x02");
        let val = parser.next_value().unwrap();
        if let Value::Map(val) = val {
            assert_eq!(val.len(), 2);
            assert_eq!(
                val.get(0).unwrap().to_owned(),
                (Value::Str("a".to_owned()), Value::Char(1))
            );
            assert_eq!(
                val.get(1).unwrap().to_owned(),
                (Value::Str("b".to_owned()), Value::Char(2))
            );
        } else {
            panic!("value is not a map");
        }
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_eof() {
        let mut parser = Parser::from(b"i\x00\x0a");

        let err = parser.next_value().expect_err("UnexpectedEof Error");

        if let Error::IO(e) = err {
            if e.kind() != std::io::ErrorKind::UnexpectedEof {
                panic!("Expected UnexpectedEof Error");
            }
        } else {
            panic!("Expected UnexpectedEof Error");
        }
    }

    #[test]
    fn test_list_too_big() {
        let mut parser = Parser::from(b"A\x00\x00\x00\x04c\x01i\x00\x00\x00\x00");

        let err = parser.next_value().expect_err("DataError");
        if let Error::DataError(_) = err {
        } else {
            panic!("Expected a DataError");
        }
    }

    #[test]
    fn test_map_too_big() {
        let mut parser = Parser::from(b"M\x00\x00\x00\x04c\x01i\x00\x00\x00\x00");

        let err = parser.next_value().expect_err("DataError");
        if let Error::DataError(_) = err {
        } else {
            panic!("Expected a DataError");
        }
    }
}
