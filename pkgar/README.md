# pkgar - Package Archive

pkgar refers to three related items - the file format, the library, and the
command line executable.

The pkgar format is not designed to be the best format for all archive uses,
only the best default format for packages on Redox OS. It is reproducible,
meaning archiving a directory will produce the same results every time. It
provides cryptographic signatures and integrity checking for package files. It
also allows this functionality to be used without storing the entire package
archive, by only storing the package header. Large files, compression,
encryption, and random access are not optimized for. Little endian is currently
assumed, as well as Unix mode flags.

***This specification is currently a work in progress***

## File Format - .pkgar

pkgar is a format for packages may be delivered in a single file (.pkgar), or as
a header file (.pkgar_head) with an associated data file (.pkgar_data). The
purpose of this is to allow downloading a header only and verifying local files
before downloading file data. Concatenating the header and data files creates a
valid single file: `cat example.pkgar_head example.pkgar_data > example.pkgar`

All data specified below is little-endian.

### Header Portion

The header portion is designed to contain the data required to verify files
already installed on disk. It is signed using NaCl (or a compatible
implementation such as libsodium), and contains the blake3, offset, size, mode,
and name of each file. The user and group IDs are left out intentionally, to
support the installation of a package either as root or as a user, for example,
in the user's home directory.

#### Header Struct

The size of the header struct is 136 bytes. All fields are packed.

- signature - 512-bit (64 byte) NaCl signature of header data
- public_key - 256-bit (32 byte) NaCl public key used to generate signature
- blake3 - 256-bit (32 byte) blake3 sum of the entry data
- count - 32-bit count of entry structs, which starts immediately after header struct
- flags - 32-bit bitflags contains what data is represented

#### Data Flags

The data flags represent what data it contained, stored as 32 bitflags.

- bit 0-8, enumeration from 0-255 represent the data and entry struct version:
  - `0`: initial version
  - others: reserved
- bit 9-16, enumeration from 0-255 represent the binary achitecture it contains:
  - `0`: architecture-independent
  - `1`: x86_64, base arch (x86_64-v1)
  - `2`: 32 bit x86, base arch (i586)
  - `3`: aarch64, base arch (Armv8-A)
  - `4`: riscv64, base arch (extension GC)
  - `17`: x86_64, v3 arch (x86_64-v3)
  - `19`: aarch64, v8.2-A arch with optional extensions (Armv8.2-A+dotprod+fp16)
  - others: reserved
- bit 17-24, enumeration from 0-255 represent how the data file is packaged:
  - `0`: not compressed
  - `1`: LZMA2, per-entry data file compression
  - others: reserved
- bit 25-31, reserved

#### Entry Struct

The size of the entry struct is 308 bytes. All fields are packed.

- blake3 - 256-bit (32 byte) blake3 sum of the file data
- offset - 64-bit offset of file data in the data portion
- size - 64-bit size in bytes of the file data in the data portion
- mode - 32-bit Unix permissions (user, group, other with read, write, execute)
- path - 256 byte NUL-terminated relative path from extract directory

### Data Portion

The data portion is used to look up file data only. It could be compressed to
produce a .pkgar_data.gz file, for example. It can be removed after the install
is completed. It is possible for it to contain holes, invalid data, or
unreferenced data - so long as the blake3 of files identified in the header are
still valid. This data should be removed when an archive is rebuilt.

The data format depends on the package format:
- `0`: Raw data.
- `1`: 64-bit uncompressed size, followed by LZMA2 compressed data.

### Operation

A reader should first verify the header portion's signature matches that of a
valid package source. Then, they should locate the entry for the file of
interest. If desired, they can check if a locally cached file matches the
referenced blake3. If this is not the case, they may access the data portion and
verify that the data at the offset and length in the header entry matches the
blake3. In that case, the data may be retrieved.

## Development
To run the integration tests, you'll need to have pkgar-keys in your $PATH (or the
$PATH of the test script). Clone the repo from
[https://gitlab.redox-os.org/MggMuggins/pkgar-keys]() and run `cargo install --path .`.
Use `test.sh` to run the integration tests.

