# File Structure

An mbon file is made up of two parts: A header, and the content.

## Header

The header begins with this 8-byte signature (hex)`EE 6D 62 6F 6E 0D 0A 00`.
After the signature is a single byte version number, currently (hex)`01`.

The signature comes from the PNG signature and the explanation will be repeated
here.

> This signature both identifies the file as a PNG file and provides for
> immediate detection of common file-transfer problems. The first two bytes
> distinguish PNG files on systems that expect the first two bytes to identify
> the file type uniquely. The first byte is chosen as a non-ASCII value to
> reduce the probability that a text file may be misrecognized as a PNG file;
> also, it catches bad file transfers that clear bit 7. Bytes two through four
> name the format. The CR-LF sequence catches bad file transfers that alter
> newline sequences. The control-Z character stops file display under MS-DOS.
> The final line feed checks for the inverse of the CR-LF translation problem. 

The 8-byte signature of mbon in decimal, hex and ascii is

```
(dec)    238  109   98  111  110   13   10    0
(hex)     EE   6D   62   6F   6E   0D   0A   00
(ascii) \356    m    b    o    n   \r   \n   \0
```

Like the png signature, this signature aims to have a unique value that can be
used to know that the file is an mbon file without relying on an extension as
well as determine if the file has encountered common problems in transfer.

After the header immediately begins the main content of the file.

## Content

The content of an mbon file is a sequence of items much like a [List]. Along
with the items can be [Heap] items. These are hidden from the user, but is used
to store data outside of the main item tree.

If heaps are used, they should have padding that would allow for the main item
tree to grow.

Descriptions of the different types available are available at
[types](types.md).

[Null]:     types.md#null
[Unsigned]: types.md#unsigned
[Signed]:   types.md#signed
[Float]:    types.md#float
[Char]:     types.md#char
[String]:   types.md#string
[Array]:    types.md#array
[List]:     types.md#list
[Struct]:   types.md#struct
[Map]:      types.md#map
[Enum]:     types.md#enum
[Space]:    types.md#space
[Padding]:  types.md#padding
[Pointer]:  types.md#pointer
[Rc]:       types.md#rc
[Heap]:     types.md#heap
