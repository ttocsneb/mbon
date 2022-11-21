use core::str;

use crate::{
    data::{Mark, Type, Value},
    error::Error,
    object::ObjectParse,
};
use byteorder::{BigEndian, ReadBytesExt};
use serde::de::DeserializeOwned;

/// A struct that parses binary data from a bytearray
///
/// You can deserialize data using
/// * `next`
/// * `next_obj`
///
/// Or you can deserialize data directly using
/// * `next_value`
pub struct Parser<'a>(&'a [u8]);

impl<'a> Parser<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }

    /// Parse the next item in the parser.
    ///
    /// ```
    /// use mbon::parser::Parser;
    ///
    /// let mut parser = Parser::new(b"i\x00\x00\x00\x42");
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
    /// Parser::new(
    ///     b"o\x00\x00\x00\x12i\x00\x00\x00\x42s\x00\x05Hellof\x00\x00\x00\x00"
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

    /// Get the next type in the parser
    ///
    /// If None is returned, then the parser is empty
    ///
    /// ```
    /// use mbon::parser::Parser;
    /// use mbon::data::Type;
    ///
    /// let parser = Parser::new(b"");
    /// assert_eq!(parser.peek_type().unwrap(), None);
    ///
    /// let parser = Parser::new(b"i\x00\x00\x00\x42");
    /// assert_eq!(parser.peek_type().unwrap(), Some(Type::Int));
    ///
    /// ```
    pub fn peek_type(&self) -> Result<Option<Type>, Error> {
        if let Some(t) = self.0.first() {
            Ok(Some(Type::from_prefix(*t)?))
        } else {
            Ok(None)
        }
    }

    fn next_type(&mut self) -> Result<Type, Error> {
        if let Some(t) = self.0.first() {
            self.0 = &self.0[1..];
            Type::from_prefix(*t)
        } else {
            Err(Error::EndOfFile)
        }
    }

    fn next_data_n(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.0.len() < n {
            return Err(Error::EndOfFile);
        }
        let val = &self.0[..n];
        self.0 = &self.0[n..];
        Ok(val)
    }

    #[inline]
    fn next_data_long(&mut self) -> Result<i64, Error> {
        Ok(self.next_data_n(8)?.read_i64::<BigEndian>()?)
    }

    #[inline]
    fn next_data_int(&mut self) -> Result<i32, Error> {
        Ok(self.next_data_n(4)?.read_i32::<BigEndian>()?)
    }

    #[inline]
    fn next_data_short(&mut self) -> Result<i16, Error> {
        Ok(self.next_data_n(2)?.read_i16::<BigEndian>()?)
    }

    #[inline]
    fn next_data_char(&mut self) -> Result<i8, Error> {
        Ok(self.next_data_n(1)?.read_i8()?)
    }

    #[inline]
    fn next_data_float(&mut self) -> Result<f32, Error> {
        Ok(self.next_data_n(4)?.read_f32::<BigEndian>()?)
    }

    #[inline]
    fn next_data_double(&mut self) -> Result<f64, Error> {
        Ok(self.next_data_n(8)?.read_f64::<BigEndian>()?)
    }

    #[inline]
    fn next_data_bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        self.next_data_n(n)
    }

    #[inline]
    fn next_data_str(&mut self, n: usize) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.next_data_n(n)?)?)
    }

    fn next_data_enum(&mut self, m: &Mark) -> Result<(u32, Value), Error> {
        let variant = self.next_data_int()? as u32;
        let value = self.next_data_value(m)?;
        Ok((variant, value))
    }

    fn next_data_array(&mut self, len: usize, t: &Mark) -> Result<Vec<Value>, Error> {
        let mut arr = Vec::with_capacity(len);

        for _ in 0..len {
            arr.push(self.next_data_value(t)?)
        }

        Ok(arr)
    }

    fn next_data_list(&mut self, size: usize) -> Result<Vec<Value>, Error> {
        let mut embed = Self::new(&self.0[..size]);
        let mut arr = Vec::new();

        while !embed.is_empty() {
            arr.push(embed.next_value()?);
        }

        self.0 = &self.0[size..];

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
        let mut embed = Self::new(&self.0[..size]);
        let mut arr = Vec::new();

        while !embed.is_empty() {
            let key = embed.next_value()?;
            let val = embed.next_value()?;
            arr.push((key, val));
        }

        self.0 = &self.0[size..];

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
            Mark::Bytes(n) => Value::Bytes(self.next_data_bytes(*n)?.to_vec()),
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
            Type::Bytes => Mark::Bytes(self.next_data_short()? as usize),
            Type::Str => Mark::Str(self.next_data_short()? as usize),
            Type::Object => Mark::Object(self.next_data_int()? as usize),
            Type::Enum => Mark::Enum(Box::new(self.next_mark()?)),
            Type::Null => Mark::Null,
            Type::Array => {
                let mark = self.next_mark()?;
                let len = self.next_data_short()? as usize;
                Mark::Array(len, Box::new(mark))
            }
            Type::List => Mark::List(self.next_data_int()? as usize),
            Type::Dict => {
                let k = self.next_mark()?;
                let v = self.next_mark()?;
                let len = self.next_data_short()? as usize;
                Mark::Dict(len, Box::new(k), Box::new(v))
            }
            Type::Map => Mark::Map(self.next_data_int()? as usize),
        })
    }

    /// Skip the next value in the parser.
    ///
    /// This will efficiently skip the next value without reading more than
    /// what's necessary.
    ///
    /// ```
    /// use mbon::parser::Parser;
    ///
    /// let mut parser = Parser::new(
    ///     b"s\x00\x1eI don't care about this stringi\x00\x00\x00\x42"
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
        if self.0.len() < size {
            return Err(Error::EndOfFile);
        }

        self.0 = &self.0[size..];
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
    /// let mut parser = Parser::new(b"i\x00\x00\x00\x42");
    ///
    /// assert_eq!(parser.next_value().unwrap(), Value::Int(0x42));
    /// ```
    pub fn next_value(&mut self) -> Result<Value, Error> {
        let mark = self.next_mark()?;
        self.next_data_value(&mark)
    }

    /// Check if the parser has any more data in it.
    #[inline]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_long() {
        let mut parser = Parser::new(b"l\x00\x30\x00\x00\x20\x10\x00\x05");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Long(0x0030000020100005));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_int() {
        let mut parser = Parser::new(b"i\x03\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Int(0x03000000));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_short() {
        let mut parser = Parser::new(b"h\x03\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Short(0x0300));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_char() {
        let mut parser = Parser::new(b"c\x03");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Char(0x03));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_float() {
        let mut parser = Parser::new(b"f\x00\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Float(0.0));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_double() {
        let mut parser = Parser::new(b"d\x00\x00\x00\x00\x00\x00\x00\x00");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Double(0.0));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_bytes() {
        let mut parser = Parser::new(b"b\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Bytes(b"Hello World".to_vec()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_str() {
        let mut parser = Parser::new(b"s\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Str("Hello World".to_owned()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_object() {
        let mut parser = Parser::new(b"o\x00\x00\x00\x0bHello World");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Object(b"Hello World".to_vec()));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_enum() {
        let mut parser = Parser::new(b"ei\x00\x00\x00\x01\x00\x00\x00\xfe");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Enum(1, Box::new(Value::Int(0xfe))));
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_null() {
        let mut parser = Parser::new(b"n");
        let val = parser.next_value().unwrap();
        assert_eq!(val, Value::Null);
        assert_eq!(parser.0.is_empty(), true);
    }

    #[test]
    fn test_array() {
        let mut parser = Parser::new(b"ac\x00\x04\x01\x02\x03\x04");
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
        let mut parser = Parser::new(
            b"aac\x00\x05\x00\x03\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f",
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
        let mut parser = Parser::new(b"A\x00\x00\x00\x08c\x01c\x02c\x03c\x04");
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
        let mut parser = Parser::new(b"M\x00\x00\x00\x0cs\x00\x01ac\x01s\x00\x01bc\x02");
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
}
