# File Structure

An mbon file is made up of two parts: A header, and the content.

## Header

The header begins with this 8-byte signature (hex)`EE 6D 62 6E 0D 0A 1A 00`.
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
(dec)    238  109   98  110   13   10   26    0
(hex)     EE   6D   62   6E   0D   0A   1A   00
(ascii) \356    m    b    n   \r   \n \032   \0
```

Like the png signature, this signature aims to have a unique value that can be
used to know that the file is an mbon file without relying on an extension as
well as determine if the file has encountered problems in transfer.

After the header immediately begins the main content of the file.

## Content

The content of an mbon file may be a sequence of items much like a list. After
the primary contents of the file may be a heap of pointer data. This is a single
heap value.

In order to allow for the main content to grow, it is recommended that the
heap leaves a buffer of reserved data at the beginning of the heap.

Descriptions of the different types available are available at
[types.md](types.md).
