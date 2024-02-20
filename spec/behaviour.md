# Behaviour

Mbon is meant to be used as a dynamic file format. Where only parts of a file
are loaded into memory and changes can be made to the file while in operation. 
I/O operations can take a long time and so they are used efficiently. 

Because of this, there are some guidelines on how mbon files should be
read/written to depending on their modes of access.

## In-memory mode

The simplest way to access an mbon file is to have it fully in memory. This is
closer to any other conventional file format where the whole file needs to be
parsed into memory. Serde implementations would be operating in this mode. 

This is most useful for data being transferred over the network, or small files.

## Read-only mode

This mode allows for multiple processes to access the file at the same time and
the file does not need to be completely loaded into memory. 

In order to open in read-only mode, there must not be a write lock on the file.
To prevent other processes from opening the file in write mode, a read lock
should be created. This is a file called `{filename}.read.lock`. The contents of
which is the number of processes that are reading the file. If the read lock
already exists, then the contents should be incremented by one. When finished,
the contents of the read lock should be decremented by 1. If the value is 0,
then the read lock can be safely deleted.

This mode is most useful when the file being read is large and no changes will
be made to it, such as with static data files.

### Reading

When reading from a file, there should be an in-memory cache of the file. This
cache should contain the data of what has been read from the file. When an item
is requested, the cache should be checked first before seeking the file. 

The user will provide the location of the item they want to read. Depending on
the type of item, specific indexes or sections of the item may be requested. The
engine may skip through items until the requested item is found.

Since disks will read one sector at a time, it is recommended that when
performing a read, the sector (usually 512 bytes) should be cached to prevent
future reads from disk.


## Read-write mode

This mode allows for a single process to access the file and can be
simultaneously read/written from.

In order to open in read-write mode, there must not be a read lock on the file
nor a write lock. If it can be opened, then a write lock file can be created.
This file is named `{filename}.write.lock`. It has no contents. When finished
with the file, then the write lock may be deleted.

This mode is most useful when the file being accessed is large and is being
updated, such as with game save states.

### Writing

Writing to a file can be complicated. When possible, large sections of the file
should not be moved around. If a change to an item caused it to shrink and items
after it would shift to the left, padding can be inserted to maintain the items'
positions. If a change to an item caused it to grow and items would shift to the
right, the changed item can be moved to the heap.

Much more advanced logic can be designed to minify the number of writes that
need to be made, but This simple algorithm should be sufficient for most
applications.
