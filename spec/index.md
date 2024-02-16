# MBON

MBON stands for Marked Binary Object Notation.

It is a file format that aims to be very efficient for reading/writing data to
disk. Portions of the file may be skipped without needing to parse everything in
between. Files can be written without truncating the file on each change. 

This document will discuss how the format is structured as well as how
implementations should behave with the data. 

* [File Structure](file-structure.md)
* [Types](types.md)


