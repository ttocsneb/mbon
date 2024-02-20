# Examples

In this document, I will provide a few examples of mbon files and how they work.

> File blocks will be written with a combination of hex and ascii. A hex
> byte will always be 2 characters long: `ab`, while an ascii byte will
> always be 1 `h`. Each byte is separated by a space
>
> ```
> h e l l o 20 w o r l d !
> ```

> Here I will be describing mbon files in a high level similar to yaml.


## Simple Example


```
list:
- unsigned<16>(1234)
- string("Hello World!")
- array<unsigned<8>>("My binary data")
```

```
EE m b o n 0D 0A 00
65 D2 04
54 12 H e l l o 20 W o r l d !
40 64 14 M y 20 b i n a r y 20 d a t a 
```

## Map

```
map:
  string("val"): unsigned<16>(1234)
  string("str"): string("Hello World!")
  string("bin"): array<unsigned<8>>("My binary data")
```

```
EE m b o n 0D 0A 00
4c 31
    54 03 v a l  65 D2 04
    54 03 s t r  54 0C H e l l o 20 W o r l d !
    54 03 b i n  40 64 0E M y 20 b i n a r y 20 d a t a 
```

## Pointers

This example is the same as above, but all the keys of the map are pointers to
rc's of strings.

```
map:
  string("val"): unsigned<16>(1234)
  string("str"): string("Hello World!")
  string("bin"): array<unsigned<8>>("My binary data")
```

```
EE m b o n 0D 0A 00
4c 28
    28 4d  65 D2 04
    28 46  54 0C H e l l o 20 W o r l d !
    28 3f  40 64 0E M y 20 b i n a r y 20 d a t a 
10 20 
    04 09 00 00 00 00 00 00 00 00 00
    2C 54 03 b i n 
    2C 54 03 s t r 
    2C 54 03 v a l
```

