//! # Marked Binary Object Notation
//!
//! mbon is a binary notation that is inspired by the NBT format.
//!
//! It is formed of a sequence of strongly typed values. Each made up of two
//! parts: a mark which defines the type and size of the data, followed by the
//! data. Marks can be different in size and so a single byte prefix is used to
//! differenciate between types.
//!
//! This format is self-describing which means that it is able to know if the
//! data is not formatted correctly or a different type was stored than what
//! was expected. Another feature of the self-describing nature of the format
//! is that you can skip values in the data without the need to parse the complete
//! item, e.g. A 1GB value can be easily skipped by only reading the mark.
//!
//! ## Usage
//!
//! ### Dumping
//!
//! You can dump binary data using the [dumper::Dumper] struct. You can
//! write values directly or use serde's serialize to write more complex data.
//!
//! ```
//! use mbon::dumper::Dumper;
//!
//! let a = 32;
//! let b = "Hello World";
//! let c = b'a';
//!
//! let mut dumper = Dumper::new();
//! dumper.write_int(a).unwrap();
//! dumper.write(&b).unwrap();
//! dumper.write(&c).unwrap();
//!
//! let output = dumper.writer();
//! assert_eq!(output, b"i\x00\x00\x00\x20s\x00\x00\x00\x0bHello Worldca");
//! ```
//!
//! ### Parsing
//!
//! You can parse binary data using the [parser::Parser] struct. You can
//! parse Value's directly, but it is recommended to use serde to parse data.
//!
//! ```
//! use mbon::parser::Parser;
//! use mbon::data::Value;
//!
//! let data = b"i\x00\x00\x00\x20s\x00\x00\x00\x0bHello Worldca";
//!
//! let mut parser = Parser::from(data);
//!
//! let a = parser.next_value().unwrap();
//! let b: String = parser.next().unwrap();
//! let c: u8 = parser.next().unwrap();
//!
//! if let Value::Int(a) = a {
//!     assert_eq!(a, 32);
//! } else {
//!     panic!("a should have been an int");
//! }
//!
//! assert_eq!(b, "Hello World");
//! assert_eq!(c, b'a');
//! ```
//!
//! ### Embedded Objects
//!
//! If you are wanting to embed a predefined object inside the format, you can
//! impl [object::ObjectDump]/[object::ObjectParse]. Keep in mind that you will
//! need to call [`write_obj()`][write_obj]/[`next_obj()`][next_obj] to take
//! advantage of it.
//!
//! [write_obj]: dumper::Dumper::write_obj
//! [next_obj]: parser::Parser::next_obj
//!
//! ```
//! use mbon::parser::Parser;
//! use mbon::dumper::Dumper;
//! use mbon::error::Error;
//! use mbon::object::{ObjectDump, ObjectParse};
//!
//! #[derive(Debug, PartialEq, Eq)]
//! struct Foo {
//!     a: i32,
//!     b: String,
//!     c: char,
//! }
//!
//! impl ObjectDump for Foo {
//!     type Error = Error;
//!
//!     fn dump_object(&self) -> Result<Vec<u8>, Self::Error> {
//!         let mut dumper = Dumper::new();
//!
//!         dumper.write(&self.a)?;
//!         dumper.write(&self.b)?;
//!         dumper.write(&self.c)?;
//!
//!         Ok(dumper.writer())
//!     }
//! }
//!
//! impl ObjectParse for Foo {
//!     type Error = Error;
//!
//!     fn parse_object(object: &[u8]) -> Result<Self, Self::Error> {
//!         let mut parser = Parser::from(object);
//!
//!         let a = parser.next()?;
//!         let b = parser.next()?;
//!         let c = parser.next()?;
//!
//!         Ok(Self { a, b, c })
//!     }
//! }
//!
//! let foo = Foo { a: 32, b: "Hello World".to_owned(), c: '🫠' };
//! let mut dumper = Dumper::new();
//!
//! dumper.write_obj(&foo).unwrap();
//!
//! let buf = dumper.writer();
//! let mut parser = Parser::from(&buf);
//!
//! let new_foo: Foo = parser.next_obj().unwrap();
//!
//! assert_eq!(foo, new_foo);
//! ```

pub mod async_wrapper;
pub mod data;
pub mod dumper;
pub mod error;
pub mod object;
pub mod parser;
