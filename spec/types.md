# Data types

In mbon, data is made out of items. These items are made of two parts: A
mark, and a value. Unless otherwise specified, an item is always a mark followed
by data. 

> All grammars in this document are written with the [pest] grammar language.

[pest]: https://pest.rs/book/

> There are several blocks of code which are written in a pseudo-code of rust.
> It will use a familiar rust syntax, but will likely not compile.

## Size

Some marks have a size indicator. This indicator is dynamically sized. The
indicator is formatted as follows: 

The indicator starts at one byte in length. There is a continuation bit in each
byte of the indicator. This is the most significant bit in each byte. If it is
1, then there is more to read, otherwise the size indicator is finished.

When reading a size indicator, the most significant bit of each byte is ignored.
The value is read as a little-endian unsigned integer. Overall, sizes may not be
larger than 64 bits or 10 characters.

### Size Grammar

```rust
SizeEnd      = { '\x00'..'\x7f' } // 0b0000_0000 through 0b0111_1111
SizeContinue = { '\x80'..'\xff' } // 0b1000_0000 through 0b1111_1111
Size = { SizeContinue ~ Size | SizeEnd }
```

### Examples

Given the data (hex)`5a b3 06`, We would first read `5a` which is (bin)`0
1011010`. We add `0b1011010 << (0 * 7)` to the sum and get `0x5a`. The Most
significant bit is 0, so we are done with a final size of 90.

Given the data (hex)`b3 06`, We read `b3` (bin)`1 0110011`. We add 
`0b0110011 << (0 * 7)` to the sum and get `0x33`. The most significant bit is 1,
so we read the next byte (hex)`06` (bin)`0 0000011`. We add 
`0b0000011 << (1 * 7)` to the sum and get `0x1b3`. The most significant bit is
0, so we are done with a final size of 435.

## IDs

Every item has an id to identify its type. This is a single byte which is used
to know what the type is. There are five parts to an id:  E, P, S, T and B.

* E bit 7: whether there is a body associated with the type.
* P bit 6: whether the type is publicly available.
* S bit 5: whether the type has a fixed size.
* T bits 2-4: the type id (which is only unique to each E, P, S combination)
* B bits 0-1: The number of bytes in the fixed size value (which is `2^B`).

Below is a diagram of how the bits are structured in the id byte as well as some
pseudo-code definitions that will be used in type descriptions.

```
7 6 5 432 10
E P S TTT BB
```

```rust
let E = 1u8 << 7;
let P = 1u8 << 6;
let S = 1u8 << 5;
let T = |v: u8| (v << 2) & 0b0001_1100;
let B = |v: u8| v & 0b0000_0011;
let b_iter = |r: Range<u8>, id: u8| r.map(|v| id | B(v)).collect::<Vec<u8>>();
let len_b = |id: u8| 2u8.pow(id & 0b0000_0011);
```

## Types

[Null]: #null
[Unsigned]: #unsigned
[Signed]: #signed
[Float]: #float
[Char]: #char
[String]: #string
[Array]: #array
[List]: #list
[Struct]: #struct
[Map]: #map
[Enum]: #enum
[Space]: #space
[Padding]: #padding
[Pointer]: #pointer
[Rc]: #rc
[Heap]: #heap

Below are definitions of all the mbon types. 

### Null

A null data type is represented by the id (hex)`c0`. There is nothing more to
the mark.

There is no data associated with the null type.

```rust
let id = E | P | T(0); // 0xc0
let len = 0;
```

#### Null Grammar

```rust
MarkNull = { "\xc0" }
```

### Unsigned

The unsigned data type is represented by the ids (hex)`64 65 66 67`. There is
nothing more to the mark.

The data is a little-endian unsigned integer of `len_b(id)` bytes.

* `64`: 1-byte (u8)
* `65`: 2-byte (u16)
* `66`: 4-byte (u32)
* `67`: 8-byte (u64)

```rust
let id = b_iter(0..4, P | S | T(1)); // [0x64, 0x65, 0x66, 0x67]
let len = len_b(id);
```

#### Unsigned Grammar

```rust
MarkU8  = { "\x64" }
MarkU16 = { "\x65" }
MarkU32 = { "\x66" }
MarkU64 = { "\x67" }
MarkUnsigned = { MaarkU8 | MarkU16 | MarkU32 | MarkU64 }
```

### Signed

The signed data type is represented by the ids (hex)`68 69 6a 6b`. There is
nothing more to the mark.

The data is a little-endian signed integer of `len_b(id)` bytes.

* `68`: 1-byte (i8)
* `69`: 2-byte (i16)
* `6a`: 4-byte (i32)
* `6b`: 8-byte (i64)

```rust
let id = b_iter(0..4, P | S | T(2)); // [0x68, 0x69, 0x6a, 0x6b]
let len = len_b(id);
```

#### Signed Grammar

```rust
MarkI8  = { "\x68" }
MarkI16 = { "\x69" }
MarkI32 = { "\x6a" }
MarkI64 = { "\x6b" }
MarkSigned = { MaarkI8 | MarkI16 | MarkI32 | MarkI64 }
```

### Float

The signed data type is represented by the ids (hex)`6e 6f`. There is
nothing more to the mark.

The data is a little-endian IEEE-754 float of `len_b(id)` bytes.

* `6e`: 4-byte (f32)
* `6f`: 8-byte (f64)

```rust
let id = b_iter(2..4, P | S | T(3)); // [0x6e, 0x6f]
let len = len_b(id);
```

#### Float Grammar

```rust
MarkF32 = { "\x6e" }
MarkF64 = { "\x6f" }
MarkFloat = {  MarkF32 | MarkF64 }
```

### Char

The char data type is represented by the ids (hex)`70 71 72`. There is nothing
more to the mark.

The data is a little-endian unsigned integer of `len_b(id)` bytes which represent
a UTF code point.

* `70`: 1-byte (u8 char)
* `71`: 2-byte (u16 char)
* `72`: 4-byte (u32 char)

```rust
let id = b_iter(2..3, P | S | T(4)); // [0x70, 0x71, 0x72]
let len = len_b(id);
```

#### Char Grammar

```rust
MarkC8  = { "\x70" }
MarkC16 = { "\x71" }
MarkC32 = { "\x72" }
MarkChar = {  MarkC8 | MarkC16 | MarkC32 }
```

### String

A string data type is represented by the id (hex)`54`. After the id, is a size
indicator we will call `L`.

The data represented by a string is a UTF-8 encoded string of `L` bytes.

```rust
let id = P | T(5); // 0x54
let len = L;
```

#### String Grammar

```rust
MarkString = { "\x54" ~ Size }
```

### Array

An array data type is represented by the id (hex)`40`. After the id is a
recursive mark we will call `V`. After `V` is a size indicator we will call `N`.

The data represented by an array is a sequence of `N` data items of type `V`. No
marks are required for each sub-item since it has already been defined by `V`.

Note that all values in the array must be homogeneous. This severely limits what
can be used for an array. If an item cannot be stored in an array, then [List]
should be used instead.

```rust
let id = P | T(0); // 0x40
let len = data_len(V) * N;
```

#### Array Grammar

```rust
MarkArray = { "\x40" ~ Mark ~ Size }
```

### List

A list data type is represented by the id (hex)`44`. After the id is a size
indicator we will call `L`.

The data represented by a list is a sequence of items where the total size of
all the items add up to `L` e.g. The contents of the list must be exactly `L`
bytes long.

```rust
let id = P | T(1); // 0x44
let len = L;
```

#### List Grammar

```rust
MarkList = { "\x44" ~ Size }
```

### Struct

A struct data type is represented by the id (hex)`40`. After the id is two marks
we will call `K` and `V` respectively. After `V` is a size indicator we will
call `N`.

The data represented by a struct is a sequence of `N` pairs of `K`-`V` data
items. No marks are required for each of these items since they have already
been defined by `K` and `V`. There are a total of `N * 2` items in a dict and
each pair of items are `K` then `V`.

Note that all values in the struct must be homogeneous. This severely limits
what can be used for a struct. If an item cannot be stored in a struct, then
[Map] should be used instead.

```rust
let id = P | T(2); // 0x48
let len = (data_len(K) + data_len(V)) * N;
```

#### Struct Grammar

```rust
MarkStruct = { "\x48" ~ Mark ~ Mark ~ Size }
```

### Map

A map data type is represented by the id (hex)`4c`. After the id is a size
indicator we will call `L`.

The data represented by a map is a sequence of pairs of items in a key-value
structure. There must be an even number of items in a map, and the total length
of the data must be equal to `L`.

```rust
let id = P | T(3); // 0x4c
let len = L;
```

#### Map Grammar

```rust
MarkMap = { "\x4c" ~ Size }
```

### Enum

The enum data type is represented by the ids (hex)`74 75 76`. After the id is
a recursive mark we will call `V`.

The data represented by the enum is a little-endian unsigned integer with
`len_b(id)` bytes which represents the variant of the enum. After the variant
value is the data of `V`. No mark is required since `V` has already been defined

* `74`: 1-byte (u8 variant)
* `75`: 2-byte (u16 variant)
* `76`: 4-byte (u32 variant)

```rust
let id = b_iter(0..3, P | S | T(5)); // [0x74, 0x75, 0x76]
let len = len_b(id) + data_len(v);
```

#### Enum Grammar

```rust
MarkE8  = { "\x74"}
MarkE16 = { "\x75" }
MarkE32 = { "\x76" }
MarkEnum = { (MarkE8 | MarkE16 | MarkE32) ~ Mark }
```

### Space

The space type is represented by the id (hex)`80`. There is nothing more to the
mark.

There is no data associated with space.

The space type is used as padding between items if needed. Whenever possible,
[Padding] should be preferred.

```rust
let id = E | T(0); // 0x80
let len = 0;
```

#### Space Grammar

```rust
MarkSpace = { "\x80" }
```

### Padding

The padding type is represented by the id (hex)`04`. After the id is a size
indicator we will call `L`.

The data of a reserved item is `L` bytes of unused space. The contents should
not be read from since it will be considered junk. 

```rust
let id = T(1); // 0x04
let len = L;
```

#### Padding Grammar

```rust
MarkPadding = { "\x04" ~ Size }
```

### Pointer

The pointer type is represented by the ids (hex)`28 29 2a 2b`. There is nothing
else to the mark.

The data of a pointer is a little-endian unsigned integer with `len_b(id)` bytes
which represent a location in the mbon file we will call `P`. The contents at
`P` must be the start of a valid mbon item. 

* `28`: 1-byte (u8 address)
* `29`: 2-byte (u16 address)
* `2a`: 4-byte (u32 address)
* `2b`: 8-byte (u64 address)

```rust
let id = b_iter(0..4, S | T(2)); // [0x28, 0x29, 0x2a, 0x2b]
let len = len_b(id);
```

#### Pointer Grammar

```rust
MarkP8  = { "\x28" }
MarkP16 = { "\x29" }
MarkP32 = { "\x2a" }
MarkP64 = { "\x2b" }
MarkPointer = { MarkP8 | MarkP16 | MarkP32 | MarkP64 }
```

### Rc

The rc type is represented by the ids (hex)`2c 2d 2e 2f`. After the id is a mark
we will call `V`.

The data of an rc is a little-endian unsigned integer with `len_b(id)` bytes
that represents the number of references to this item. After which is the data
for `V`. No mark is required since `V` has already been defined.

Rc's should always be used alongside [Pointer]s. They should be treated like an
invisible box most of the time; Only when doing pointer operations should rc's
be considered.

* `2c`: 1-byte (u8 reference count)
* `2d`: 2-byte (u16 reference count)
* `2e`: 4-byte (u32 reference count)
* `2f`: 8-byte (u64 reference count)

```rust
let id = b_iter(0..4, S | T(3)); // [0x2c, 0x2d, 0x2e, 0x2f]
let len = len_b(id) + data_len(V);
```

#### Rc Grammar

```rust
MarkR8  = { "\x2c" }
MarkR16 = { "\x2d" }
MarkR32 = { "\x2e" }
MarkR64 = { "\x2f" }
MarkRc = { (MarkR8 | MarkR16 | MarkR32 | MarkR64) ~ Mark }
```

### Heap

The heap type is represented by the id (hex)`10`. After the id is a size
indicator we will call `L`.

The data of the heap is a sequence of items where the total size of all the
items add up to `L`.

The contents of the heap are hidden from the user, in other words it should be
treated like padding, but with valid data inside. The only way the user can
access items in the heap is through [Pointer]s. The heap should be a root level
item of the mbon file. 

```rust
let id = T(4); // 0x10
let len = L;
```

#### Heap Grammar

```rust
MarkHeap = { "\x10" ~ Size }
```

# Full Mark Grammar

Below is a comprehensive grammar for marks in the mbon format.

```rust
SizeEnd      = { '\x00'..'\x7f' } // 0b0......
SizeContinue = { '\x80'..'\xff' } // 0b1......
Size = { SizeContinue ~ Size | SizeEnd }

Mark = {
        MarkNull 
      | MarkUnsigned | MarkSigned | MarkFloat 
      | MarkChar     | MarkString 
      | MarkArray    | MarkList 
      | MarkStruct   | MarkMap 
      | MarkEnum 
      | MarkSpace    | MarkPadding 
      | MarkPointer  | MarkRc     | MarkHeap
}

MarkNull = { "\xc0" }

MarkU8       = { "\x64" }
MarkU16      = { "\x65" }
MarkU32      = { "\x66" }
MarkU64      = { "\x67" }
MarkUnsigned = { MaarkU8 | MarkU16 | MarkU32 | MarkU64 }

MarkI8     = { "\x68" }
MarkI16    = { "\x69" }
MarkI32    = { "\x6a" }
MarkI64    = { "\x6b" }
MarkSigned = { MaarkI8 | MarkI16 | MarkI32 | MarkI64 }

MarkF32   = { "\x6e" }
MarkF64   = { "\x6f" }
MarkFloat = {  MarkF32 | MarkF64 }

MarkC8   = { "\x70" }
MarkC16  = { "\x71" }
MarkC32  = { "\x72" }
MarkChar = {  MarkC8 | MarkC16 | MarkC32 }

MarkString = { "\x54" ~ Size }

MarkArray  = { "\x40" ~ Mark ~ Size }

MarkList   = { "\x44" ~ Size }

MarkStruct = { "\x48" ~ Mark ~ Mark ~ Size }

MarkMap    = { "\x4c" ~ Size }

MarkE8   = { "\x74"}
MarkE16  = { "\x75" }
MarkE32  = { "\x76" }
MarkEnum = { (MarkE8 | MarkE16 | MarkE32) ~ Mark }

MarkSpace   = { "\x80" }

MarkPadding = { "\x04" ~ Size }

MarkP8      = { "\x28" }
MarkP16     = { "\x29" }
MarkP32     = { "\x2a" }
MarkP64     = { "\x2b" }
MarkPointer = { MarkP8 | MarkP16 | MarkP32 | MarkP64 }

MarkR8  = { "\x2c" }
MarkR16 = { "\x2d" }
MarkR32 = { "\x2e" }
MarkR64 = { "\x2f" }
MarkRc  = { (MarkR8 | MarkR16 | MarkR32 | MarkR64) ~ Mark }

MarkHeap = { "\x10" ~ Size }
```

