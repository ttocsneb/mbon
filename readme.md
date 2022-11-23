[![](https://img.shields.io/crates/v/mbon.svg)][crates-io]
[![api-docs](https://docs.rs/mbon/badge.svg)][Documentation]

[crates-io]: https://crates.io/crates/mbon
[Documentation]: https://docs.rs/mbon

# Marked Binary Object Notation

mbon is a binary notation that is inspired by the NBT format.

It is formed of a sequence of strongly typed values. Each made up of two
parts: a mark which defines the type and size of the data, followed by the
data. Marks can be different in size and so a single byte prefix is used to
differenciate between types.

This format is self-describing which means that it is able to know if the
data is not formatted correctly or a different type was stored than what
was expected. Another feature of the self-describing nature of the format
is that you can skip values in the data without the need to parse the complete
item, e.g. A 1GB value can be easily skipped by only reading the mark.

## Usage

### Dumping

You can dump binary data using the [Dumper] struct. You can write values
directly or use serde's serialize to write more complex data.

[Dumper]: https://docs.rs/mbon/latest/mbon/dumper/struct.Dumper.html

```rust
use mbon::dumper::Dumper;

let a = 32;
let b = "Hello World";
let c = b'a';

let mut dumper = Dumper::new();
dumper.write_int(a).unwrap();
dumper.write(&b).unwrap();
dumper.write(&c).unwrap();

let output = dumper.writer();
assert_eq!(output, b"i\x00\x00\x00\x20s\x00\x00\x00\x0bHello Worldca");
```

### Parsing

You can parse binary data using the [Parser] struct. You can parse Value's
directly, but it is recommended to use serde to parse data.

[Parser]: https://docs.rs/mbon/latest/mbon/parser/struct.Parser.html

```rust
use mbon::parser::Parser;
use mbon::data::Value;

let data = b"i\x00\x00\x00\x20s\x00\x00\x00\x0bHello Worldca";

let mut parser = Parser::from(data);

let a = parser.next_value().unwrap();
let b: String = parser.next().unwrap();
let c: u8 = parser.next().unwrap();

if let Value::Int(a) = a {
    assert_eq!(a, 32);
} else {
    panic!("a should have been an int");
}

assert_eq!(b, "Hello World");
assert_eq!(c, b'a');
```

### Embedded Objects

If you are wanting to embed a predefined object inside the format, you can
impl [ObjectDump]/[ObjectParse]. Keep in mind that you will need to call
write_obj/parse_obj to take advantage of it.

[ObjectDump]: https://docs.rs/mbon/0.1.1/mbon/object/trait.ObjectDump.html
[ObjectParse]: https://docs.rs/mbon/0.1.1/mbon/object/trait.ObjectParse.html

```rust
use mbon::parser::Parser;
use mbon::dumper::Dumper;
use mbon::error::Error;
use mbon::object::{ObjectDump, ObjectParse};

#[derive(Debug, PartialEq, Eq)]
struct Foo {
    a: i32,
    b: String,
    c: char,
}

impl ObjectDump for Foo {
    type Error = Error;

    fn dump_object(&self) -> Result<Vec<u8>, Self::Error> {
        let mut dumper = Dumper::new();

        dumper.write(&self.a)?;
        dumper.write(&self.b)?;
        dumper.write(&self.c)?;

        Ok(dumper.writer())
    }
}

impl ObjectParse for Foo {
    type Error = Error;

    fn parse_object(object: &[u8]) -> Result<Self, Self::Error> {
        let mut parser = Parser::new(object);

        let a = parser.next()?;
        let b = parser.next()?;
        let c = parser.next()?;

        Ok(Self { a, b, c })
    }
}

let foo = Foo { a: 32, b: "Hello World".to_owned(), c: '🫠' };
let mut dumper = Dumper::new();

dumper.write_obj(&foo).unwrap();

let buf = dumper.writer();
let mut parser = Parser::from(&buf);

let new_foo: Foo = parser.next_obj().unwrap();

assert_eq!(foo, new_foo);
```

## Grammar

Below is a grammar for the binary format. Note that all numbers are stored
in big-endian form.

```EBNF
Value  ::= long | int | short | char | float | double | null | bytes
         | str | object | enum | array | list | dict | map;
Mark   ::= Mlong | Mint | Mshort | Mchar | Mfloat | Mdouble | Mnull
         | Mbytes | Mstr | Mobject | Menum | Marray | Mlist | Mdict | Mmap;
Data   ::= Dlong | Dint | Dshort | Dchar | Dfloat | Ddouble | Dnull
         | Dbytes | Dstr | Dobject | Denum | Darray | Dlist | Ddict | Dmap;

u32 ::= <u8> <u8> <u8> <u8>;
i64 ::= <u8> <u8> <u8> <u8> <u8> <u8> <u8> <u8>;
i32 ::= <u8> <u8> <u8> <u8>;
i16 ::= <u8> <u8>;
i8  ::= <u8>;
f64 ::= <u8> <u8> <u8> <u8> <u8> <u8> <u8> <u8>;
f32 ::= <u8> <u8> <u8> <u8>;


Mlong   ::= 'l';
Mint    ::= 'i';
Mshort  ::= 'h';
Mchar   ::= 'c';
Mfloat  ::= 'f';
Mdouble ::= 'd';
Mnull   ::= 'n';
Mbytes  ::= 'b' u32;
Mstr    ::= 's' u32;
Mobject ::= 'o' u32;
Menum   ::= 'e' Mark#enum;
Marray  ::= 'a' Mark#item u32;
Mlist   ::= 'A' u32;
Mdict   ::= 'm' Mark#key Mark#value u32;
Mmap    ::= 'M' u32;

Dlong   ::= i64;
Dint    ::= i32;
Dshort  ::= i16;
Dchar   ::= i8;
Dfloat  ::= f32;
Ddouble ::= f64;
Dnull   ::= ;
Dbytes  ::= u8 Dbytes |;
Dstr    ::= u8 Dbytes |;
Dobject ::= u8 Dbytes |;
Denum   ::= u32 Data#enum;
Darray  ::= Data#item Darray |;
Dlist   ::= Value Dlist |;
Ddict   ::= Data#key Data#value Ddict |;
Dmap    ::= Value Value Dmap |;

long   ::= Mlong Dlong;
int    ::= Mint Dint;
short  ::= Mshort Dshort;
char   ::= Mchar Dchar;
float  ::= Mfloat Dflaot;
double ::= Mdouble Ddouble;
null   ::= Mnull;
bytes  ::= Mbytes Dbytes;
str    ::= Mstr Dstr;
object ::= Mobject Dbytes;
enum   ::= Menum Denum;
array  ::= Marray Darray;
list   ::= Mlist Dlist;
dict   ::= Mdict Ddict;
map    ::= Mmap Dmap;
```

> `X#name` means that it is related to any other `Y#name`, e.g. `Mark#item`
> in `Marray` relates to `Data#item`.

## Specification

| Name   | Description                       |
|--------|-----------------------------------|
| Long   | 64 bit integer                    |
| Int    | 32 bit integer                    |
| Short  | 16 bit integer                    |
| Char   | 8 bit integer                     |
| Float  | 32 bit IEEE-754 float             |
| Double | 64 bit IEEE-754 float             |
| Null   | Only the mark                     |
| Bytes  | Unencoded string of bytes         |
| Str    | UTF-8 encoded string              |
| Object | Embeded preformatted data         |
| Enum   | u32 Variant, embed data           |
| Array  | `len` list of `item` data         |
| List   | list of values                    |
| Dict   | `len` list of `key`-`value` pairs |
| Map    | list of key-value pairs           |

### Numbers

Every number is defined only by their mark. There is no additional data
stored in a number's mark.

All numbers are stored in a big endian binary form. Integers are internally
considered signed, however, there is no requirement that they need to be
signed, so it is possible to read an unsigned integer as a signed integer.

### Strings

Strings will store their type marker followed by a u32 for their length e.g.
`b"s\x00\x00\x00\x05"` would indicate a string that is 5 bytes long. Strings
must be UTF-8 encoded; If you do not want this behaviour, you can use Bytes
which behave in the same way as Str, but without the UTF-8 requirement.

### Object

If you wanted to embed binary data, you can use an object value. This is
similar to the bytes value, but it uses an unsigned int for the length. It
is meant for storing binary data with a predetermined format.

### Enum

An enum is meant to be compatible with Rust's enum serialization. It is
defined by a variant id followed by an embedded Value. To make the enum
self-describing, the mark for the embedded value is placed within the enum's
mark.

### Null

Null is only uses its mark, there is no data associated with it.

### Array

Sequences can be stored in two forms; The Array being more strict than a
list. If a sequence cannot be stored as an array, it will be stored as a
list.

An array is a sequence of items that all share the same mark. This means
that all elements must be the same size: A vector of u32's will always be
stored as an array, while a vector of strings can only be stored as an array
if each string is the same length.

The Array Mark contains the mark of the contained item followed by then the
number of items in a u16.

### List

The list is the more lenient way to store sequences. It simply holds a
sequence of all the items. The mark of a list simply holds the number of
bytes in the list as a u32.

### Dict

A dict is similar to an array, but it stores key-value pairs instead. All
keys must share the same mark and all values must share the same mark.

The Dict mark contains the key mark, followed by the value mark, and finally
the number of items in the dict.

### Map

The map, similar to the list stores any value types, but in a key-value
format. This is the fallback format if a value cannot be stored as a dict.
