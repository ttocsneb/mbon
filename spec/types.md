# Data types

In mbon, data is made out of items. These items are made of two parts: A
mark, and a value. Unless otherwise specified, an item is always a mark followed
by data. 

## Size

Some marks have a size indicator. This indicator is dynamically sized. The
indicator is formatted as follows: 

The indicator starts at one byte in length. There is a continuation bit in each
byte of the indicator. This is the most significant bit in each byte. If it is
1, then there is more to read, otherwise the size indicator is finished.

```rust
SizeEnd      = { '\x00'..'\x7f' } // 0b0000_0000 through 0b0111_1111
SizeContinue = { '\x80'..'\xff' } // 0b1000_0000 through 0b1111_1111
Size = { SizeContinue ~ Size | SizeEnd }
```

When reading a size indicator, the most significant bit of each byte is ignored.
The value is read as a little-endian unsigned integer. Overall, sizes may not be
larger than 64 bits or 10 characters.

### Examples

Given the data (hex)`5a b3 06`, We would first read `5a` which is (bin)`0
1011010`. We add `0b1011010 << (0 * 7)` to the sum and get `0x5a`. The Most
significant bit is 0, so we are done with a final size of 90.

Given the data (hex)`b3 06`, We read `b3` (bin)`1 0110011`. We add 
`0b0110011 << (0 * 7)` to the sum and get `0x33`. The most significant bit is 1,
so we read the next byte (hex)`06` (bin)`0 0000011`. We add 
`0b0000011 << (1 * 7)` to the sum and get `0x1b3`. The most significant bit is
0, so we are done with a final size of 435.

## U8

A u8 data type is represented by the id `b`. There is nothing more to the
mark. 

The data represented is an unsigned 8-bit integer.

The length of a u8 is always 1.

## I8

An i8 data type is represented by the id `B`. There is nothing more to the
mark. 

The data represented is a signed 8-bit integer.

The length of an i8 is always 1.

## U16

A u16 data type is represented by the id `h`. There is nothing more to the
mark. 

The data represented is a little-endian unsigned 16-bit integer.

The length of a u16 is always 2.

## I16

An i16 data type is represented by the id `H`. There is nothing more to the
mark. 

The data represented is a little-endian signed 16-bit integer.

The length of an i16 is always 2.

## U32

A u32 data type is represented by the id `i`. There is nothing more to the
mark. 

The data represented is a little-endian unsigned 32-bit integer.

The length of a u32 is always 4.

## I32

An i32 data type is represented by the id `I`. There is nothing more to the
mark. 

The data represented is a little-endian signed 32-bit integer.

The length of an i32 is always 4.

## U64

A u64 data type is represented by the id `l`. There is nothing more to the
mark. 

The data represented is a little-endian unsigned 64-bit integer.

The length of a u64 is always 8.

## I64

An i64 data type is represented by the id `L`. There is nothing more to the
mark. 

The data represented is a little-endian signed 64-bit integer.

The length of an i64 is always 8.

## F32

An f32 data type is represented by the id `f`. There is nothing more to the
mark. 

The data represented is a little-endian IEEE-754 float.

The length of an f32 is always 4.

## F64

An i64 data type is represented by the id `F`. There is nothing more to the
mark. 

The data represented is a little-endian IEEE-754 double.

The length of an f64 is always 8.

## Null

A null data type is represented by the id `n`. There is nothing more to the
mark. 

There is no data associated with a null.

The length of a null is always 0.

## Chars

Characters are all represented by UTF code points. A majority of english
characters fit within the 8-bit range of a byte. In many other languages, most
will fit within 16-bits. And all characters can fit within 32-bits.

It is possible to have characters represented by UTF-8, but that would require a
size indicator in the mark to know how long the character is. A better option
would be to have 3 char types each with different sizes to accommodate for all
possible characters without wasting unused space.

### Small Char

A char data type is represented by the id `c`. There is nothing more to the
mark. The data represented is a unsigned 8-bit integer which represents a UTF
code point. If the code point doesn't fit within an 8-bit value, then Char or
Big Char should be used instead.

The length of a small char is always 1.

### Char

A char data type is represented by the id `C`. There is nothing more to the
mark. The data represented is a little-endian unsigned 16-bit integer which
represents a UTF code point. If the code point doesn't fit within a 16-bit
value, then Big Char should be used instead.

The length of a char is always 2.

### Big Char

A char data type is represented by the id `G`. There is nothing more to the
mark. The data represented is a little-endian unsigned 32-bit integer which
represents a UTF code point.

The length of a big char is always 4.

## String

A string data type is represented by the id `s`. After the id, is a size
indicator we will call `L`.

The data represented by a string is a UTF-8 encoded string of `L` bytes.

The length of a string is `L`.

## Array

An array data type is represented by the id `a`. After the id is a recursive
mark we will call `I`. After `I` is a size indicator we will call `N`.

The data represented by an array is a sequence of `N` data items of type `I`. No
marks are required for each sub-item since it has already been defined by `I`.

The length of an array is `Len(I) * N`.

## List

A list data type is represented by the id `A`. After the id is a size indicator
we will call `L`.

The data represented by a list is a sequence of items where the total size of
all the items add up to `L` e.g. The contents of the list must be exactly `L`
bytes long.

The length of an list is `L`.

## Dict

A dict data type is represented by the id `d`. After the id is two marks we will
call `K` and `V` respectively. After `V` is a size indicator we will call `N`.

The data represented by a dict is a sequence of `N` pairs of `K`-`V` data
items. No marks are required for each sub-item since they have already been
defined by `K` and `V`. There are a total of `N * 2` items in a dict and each
pair of items are `K` then `V`.

The length of a dict is `(Len(K) + Len(V)) * N`.

## Map

A map data type is represented by the id `D`. After the id is a size indicator
we will call `L`.

The data represented by a map is a sequence of pairs of items in a key-value
structure. There must be an even number of items in a map, and the total length
of the data must be equal to `L`.

The length of a map is `L`.

## Small Enum

A small enum data type is represented by the id `e`. After the id is a recursive
mark we will call `V`.

The data represented by a small enum is an unsigned 8-bit integer that
represents the variant of the enum. After the variant is the data for `V`. No
mark is required since `V` has already been defined.

The length of a small enum is `1 + Len(V)`.

## Enum

An enum data type is represented by the id `E`. After the id is a recursive
mark we will call `V`.

The data represented by a small enum is a little-endian unsigned 16-bit integer
that represents the variant of the enum. After the variant is the data for `V`.
No mark is required since `V` has already been defined.

The length of an enum is `2 + Len(V)`.

## Big Enum

A big enum data type is represented by the id `U`. After the id is a recursive
mark we will call `V`.

The data represented by a small enum is a little-endian unsigned 32-bit integer
that represents the variant of the enum. After the variant is the data for `V`.
No mark is required since `V` has already been defined.

The length of an enum is `4 + Len(V)`.

## Implicit Types

There are a few types that are not exposed to the user. These are designed to
help optimize the file for I/O. A more detailed discussion about I/O
optimizations will be discussed somewhere else _TODO_.

### Small Pointer

A small pointer data type is represented by the id `p`. There is nothing more to
the mark.

The data of the small pointer is a little-endian unsigned 16-bit integer which
represents a location within the file where the value can be found.

The length of a small pointer is always 2.

### Pointer

A pointer data type is represented by the id `P`. There is nothing more to the
mark.

The data of the small pointer is a little-endian unsigned 32-bit integer which
represents a location within the file where the value can be found.

The length of a pointer is always 4.

### Big Pointer

A pointer data type is represented by the id `T`. There is nothing more to the
mark.

The data of the small pointer is a little-endian unsigned 64-bit integer which
represents a location within the file where the value can be found.

The length of a big pointer is always 8.

### Reserved

A reserved data type is represented by the id `r`. After the id is a size
indicator we will call `L`.

The data of the reserved item is unknown. This data should not be read from. The
only requirement is that there must be `L` bytes of data.

The length of reserved space is `L`.

### Empty

An empty data type is represented only by the id `\x00`. There is nothing more
to the mark and there is no data associated with an empty.

Empty is designed to be used in a similar way to reserved, but where reserved
cannot fit.

The length of empty is always `0`.

### Rc

An rc is a pointer receiver that counts how many references there are to it.

#### Small Rc

A small rc data type is represented by the id `x`. After the id is a mark we
will call `V`.

The data of a small rc is a 1-byte unsigned integer that represents the number
of references to the value. After this is the data value `V`.

The length of small rc is `1 + Len(V)`

#### Rc

An rc data type is represented by the id `X`. After the id is a mark we will
call `V`.

The data of a small rc is a 2-byte little-endian unsigned integer that
represents the number of references to the value. After this is the data value
`V`.

The length of rc is `2 + Len(V)`

#### Big Rc

An big rc data type is represented by the id `y`. After the id is a mark we will
call `V`.

The data of a small rc is a 4-byte little-endian unsigned integer that
represents the number of references to the value. After this is the data value
`V`.

The length of big rc is `4 + Len(V)`

### Heap

A heap data type is represented by the id `k`. After the id is a size indicator
we will call `L`.

The data of the heap is a sequence of reserved, empty, small rc, rc, or big rc.
This is reserved for pointer values. The contents of the heap must be exactly
`L` bytes long.

The length of the heap is `L`.

# Mark Grammar

Below is a comprehensive grammar for marks in the mbon format.

```rust
SizeEnd      = { '\x00'..'\x7f' } // 0b0......
SizeContinue = { '\x80'..'\xff' } // 0b1......
Size = { SizeContinue ~ Size | SizeEnd }

MarkU8   = { "b" }
MarkI8   = { "B" }
MarkU16  = { "h" }
MarkI16  = { "H" }
MarkU32  = { "i" }
MarkI32  = { "I" }
MarkU64  = { "l" }
MarkI64  = { "L" }
MarkF32  = { "f" }
MarkF64  = { "F" }
MarkNull = { "n" }

MarkSmallChar = { "c" }
MarkChar      = { "C" }
MarkBigChar   = { "G" }

MarkStr  = { "s" ~ Size }
MarkArr  = { "a" ~ Mark ~ Size }
MarkList = { "A" ~ Size }
MarkDict = { "d" ~ Mark ~ Mark ~ Size }
MarkMap  = { "D" ~ Size }

MarkSmallEnum = { "e" ~ Mark }
MarkEnum      = { "E" ~ Mark }
MarkBigEnum   = { "U" ~ Mark }

MarkSmallPtr = { "p" }
MarkPtr      = { "P" }
MarkBigPtr   = { "T" }

MarkReserved = { "r" ~ Size }
MarkEmpty    = { "\x00" }

MarkSmallRc  = { "x" ~ Mark }
MarkRc       = { "X" ~ Mark }
MarkBigRc    = { "y" ~ Mark }

MarkHeap     = { "k" ~ Size }

Mark = {
      MarkU8  | MarkI8 
    | MarkU16 | MarkI16 
    | MarkU32 | MarkI32 
    | MarkU64 | MarkI64
    | MarkF32 | MarkF64 
    | MarkNull 
    | MarkSmallChar | MarkChar | MarkBigChar
    | MarkStr
    | MarkArr  | MarkList
    | MarkDict | MarkMap
    | MarkSmallEnum | MarkEnum | MarkBigEnum
    | MarkSmallPtr  | MarkPtr  | MarkBigPtr
    | MarkEmpty | MarkEmtpy
    | MarkSmallRc | MarkRc | MarkBigRc
    | MarkHeap
}

```
