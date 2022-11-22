//! # Custom Object parsing and dumping
//!
//! You can implement [ObjectParse] and [ObjectDump] to allow for custom object
//! dumping and parsing.

/// A loader that can load a struct from a binary object.
///
/// A possible use case is to store a struct more efficiently than a map
///
/// ```
/// use mbon::object::ObjectParse;
/// use mbon::parser::Parser;
/// use mbon::error::Error;
///
/// struct Foo {
///     a: u32,
///     b: String,
/// }
///
/// impl ObjectParse for Foo {
///     type Error = Error;
///
///     fn parse_object(object: &[u8]) -> Result<Self, Self::Error> {
///         let mut parser = Parser::new(object);
///
///         let a = parser.next()?;
///         let b = parser.next()?;
///
///         Ok(Self { a, b })
///     }
/// }
/// ```
pub trait ObjectParse
where
    Self: Sized,
{
    type Error;

    /// Load from a binary object
    ///
    /// This will parse the given object in a predefined format.
    fn parse_object(object: &[u8]) -> Result<Self, Self::Error>;
}

/// A dumper that can dump a binary object from a struct.
///
/// A possible use case is to store a struct more efficiently than a map
///
/// ```
/// use mbon::object::ObjectDump;
/// use mbon::dumper::Dumper;
/// use mbon::error::Error;
///
/// struct Foo {
///     a: u32,
///     b: String,
/// }
///
/// impl ObjectDump for Foo {
///     type Error = Error;
///
///     fn dump_object(&self) -> Result<Vec<u8>, Self::Error> {
///         let mut dumper = Dumper::new();
///
///         dumper.write(&self.a)?;
///         dumper.write(&self.b)?;
///
///         Ok(dumper.into())
///     }
/// }
/// ```
pub trait ObjectDump {
    type Error;

    /// Dump into a binary object
    ///
    /// This will dump the struct into binary data in a predefined format
    fn dump_object(&self) -> Result<Vec<u8>, Self::Error>;
}

#[cfg(test)]
mod test {
    use std::vec;

    use crate::{dumper::Dumper, error::Error, parser::Parser};

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestStruct {
        a: String,
        b: i32,
        c: Vec<String>,
    }

    impl ObjectParse for TestStruct {
        type Error = Error;

        fn parse_object(data: &[u8]) -> Result<Self, Error> {
            let mut parser = Parser::new(data);
            let a: String = parser.next()?;
            let b: i32 = parser.next()?;
            let c: Vec<String> = parser.next()?;
            Ok(Self { a, b, c })
        }
    }

    impl ObjectDump for TestStruct {
        type Error = Error;

        fn dump_object(&self) -> Result<Vec<u8>, Error> {
            let mut dumper = Dumper::new();
            dumper.write(&self.a)?;
            dumper.write(&self.b)?;
            dumper.write(&self.c)?;
            Ok(dumper.into())
        }
    }

    #[test]
    fn test_deserialize() {
        let data =
            b"o\x00\x00\x00\x2bs\x00\x00\x00\x0bHello Worldi\x00\x00\x40\x30A\x00\x00\x00\x11s\x00\x00\x00\x04Yeets\x00\x00\x00\x03Bar";

        let mut parser = Parser::new(data);
        let test: TestStruct = parser.next_obj().unwrap();
        assert_eq!(
            test,
            TestStruct {
                a: "Hello World".into(),
                b: 0x4030,
                c: vec!["Yeet".into(), "Bar".into()]
            }
        );
    }

    #[test]
    fn test_serialize() {
        let data =
            b"o\x00\x00\x00\x2bs\x00\x00\x00\x0bHello Worldi\x00\x00\x40\x30A\x00\x00\x00\x11s\x00\x00\x00\x04Yeets\x00\x00\x00\x03Bar";

        let mut dumper = Dumper::new();
        dumper
            .write_obj(&TestStruct {
                a: "Hello World".into(),
                b: 0x4030,
                c: vec!["Yeet".into(), "Bar".into()],
            })
            .unwrap();

        assert_eq!(dumper.buffer(), data);
    }
}
