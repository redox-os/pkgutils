//! Extention traits for base types defined in `pkgar-core`.
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Seek, Take, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use blake3::{Hash, Hasher};
use pkgar_core::{Entry, Header, PackageSrc, Packaging};

use crate::{Error, wrap_io_err};

/// Handy associated functions for `pkgar_core::Entry` that depend on std
pub trait EntryExt {
    /// Iterate the components of the path and ensure that there are no
    /// non-normal components.
    fn check_path(&self) -> Result<&Path, Error>;

    /// Check if the entry path is valid and being inside of the `base`,
    /// return a new entry with path relative from `base`.
    fn sub_path(&self, base: &Path) -> Option<Self>
    where
        Self: Sized;

    /// Verify is extracted blake3 or compressed size is correct
    fn verify<R>(&self, blake3: Hash, size: u64, reader: &DataReader<R>) -> Result<(), Error>
    where
        R: Sized;
}

impl EntryExt for Entry {
    fn check_path(&self) -> Result<&Path, Error> {
        let path = Path::new(OsStr::from_bytes(self.path_bytes()));
        for component in path.components() {
            match component {
                Component::Normal(_) => {}
                invalid => {
                    let bad_component: &Path = invalid.as_ref();
                    return Err(Error::InvalidPathComponent {
                        path: path.to_path_buf(),
                        invalid: bad_component.to_path_buf(),
                        entry: Some(Box::new(*self)),
                    });
                }
            }
        }
        Ok(path)
    }

    fn verify<R>(&self, blake3: Hash, size: u64, reader: &DataReader<R>) -> Result<(), Error> {
        if size != reader.unpacked_size && reader.unpacked_size != u64::MAX {
            Err(Error::LengthMismatch {
                actual: size,
                expected: reader.unpacked_size,
            })
        } else if self.size() != reader.data_size {
            Err(Error::LengthMismatch {
                actual: self.size(),
                expected: reader.data_size,
            })
        } else if blake3 != self.blake3() {
            Err(pkgar_core::Error::InvalidBlake3.into())
        } else {
            Ok(())
        }
    }

    fn sub_path(&self, base: &Path) -> Option<Self> {
        let path = self.check_path().ok()?;
        let split = path.strip_prefix(base).ok()?;
        let split_bytes = split.to_str()?.as_bytes();
        let mut new = *self;
        new.path = [0u8; _];
        new.path
            .get_mut(..split_bytes.len())?
            .copy_from_slice(split_bytes);
        Some(new)
    }
}

pub trait PackageSrcExt<R>
where
    Self: PackageSrc + Sized,
    R: Read + Seek,
{
    /// Get the path corresponding to this `PackageSrc`.
    fn path(&self) -> std::borrow::Cow<'_, str>;

    /// Take the underlying reader out into the data reader
    fn take_reader(&mut self) -> Result<R, Error>;

    /// Put the underlying reader back in from the data reader
    fn restore_reader(&mut self, reader: R) -> Result<(), Error>;

    /// Build a reader for a given entry on this source.
    /// Must call reader.finish() before getting another reader.
    fn data_reader(&mut self, entry: &Entry) -> Result<DataReader<R>, Error> {
        let mut reader = self.take_reader()?;
        let offset = self.header().total_size()? as u64 + entry.offset;
        reader
            .seek(io::SeekFrom::Start(offset))
            .map_err(wrap_io_err!("Seeking for data reader"))?;
        DataReader::new(&self.header(), reader, entry.size)
            .map_err(wrap_io_err!("Seeking for data reader"))
    }
}

/// Copy the contents of `read` into `write` by streaming through buf.
/// The basic function of this function is analogous to io::copy, except it
/// outputs the blake3 hash of the data streamed, and also does not allocate.
pub(crate) fn copy_and_hash<R: Read, W: Write>(
    read: &mut R,
    write: &mut W,
    buf: &mut [u8],
) -> Result<(u64, Hash), io::Error> {
    let mut hasher = Hasher::new();
    let mut written = 0;
    loop {
        let count = read.read(buf)?;
        if count == 0 {
            break;
        }
        written += count as u64;
        hasher.update_rayon(&buf[..count]);

        write.write_all(&buf[..count])?;
    }
    Ok((written, hasher.finalize()))
}

/// Create a diff between two entries, returns (entries_to_install, entries_to_update(old, new), entries_to_remove, skipped_entries_count)
pub(crate) fn diff_package(
    old_entries: &[Entry],
    new_entries: &[Entry],
) -> Result<(Vec<Entry>, Vec<(Entry, Entry)>, Vec<Entry>, usize), Error> {
    let mut old_map = BTreeMap::new();
    for entry in old_entries {
        old_map.insert(entry.check_path()?.to_path_buf(), entry);
    }
    let mut entries_to_install = Vec::new();
    let mut entries_to_update = Vec::new();
    let mut skipped_entries_count = 0;
    for entry in new_entries {
        let path = entry.check_path()?;
        old_map.remove(path);

        match old_map.get(path) {
            Some(old_hash) if old_hash.blake3() == entry.blake3() => {
                skipped_entries_count += 1;
            }
            Some(old) => {
                entries_to_update.push((**old, *entry));
            }
            None => {
                entries_to_install.push(*entry);
            }
        }
    }
    let mut entries_to_remove = Vec::new();
    for old_e in old_map.into_values() {
        entries_to_remove.push(*old_e);
    }
    Ok((
        entries_to_install,
        entries_to_update,
        entries_to_remove,
        skipped_entries_count,
    ))
}

/// Implements reader based on data flags
pub struct DataReader<R> {
    pub data_size: u64,
    pub unpacked_size: u64,
    pub inner: DataReaderKind<R>,
}

impl<R: Read + Seek> DataReader<R> {
    pub fn packaging_type(&self) -> pkgar_core::Packaging {
        match self.inner {
            DataReaderKind::Uncompressed(_) => Packaging::Uncompressed,
            DataReaderKind::LZMA2(_) => Packaging::LZMA2,
        }
    }
}
#[expect(clippy::large_enum_variant, reason = "too short-lived to use heap")]
pub enum DataReaderKind<R> {
    Uncompressed(Take<R>),
    LZMA2(lzma_rust2::Lzma2Reader<Take<R>>),
}

impl<R: Read + Seek> DataReader<R> {
    pub fn new(header: &Header, mut file: R, len: u64) -> std::io::Result<Self> {
        let mut unpacked_size = len;
        let inner = match header.flags.packaging() {
            Packaging::LZMA2 => {
                let mut ulen_buf = [0u8; size_of::<u64>()];
                // failure to read len is allowed for "verify" purpose
                if file.read_exact(&mut ulen_buf).is_ok() {
                    unpacked_size = u64::from_le_bytes(ulen_buf);
                } else {
                    unpacked_size = u64::MAX;
                }
                let decoder = lzma_rust2::Lzma2Reader::new(
                    file.take(len),
                    // same dict size with writer
                    lzma_rust2::LzmaOptions::DICT_SIZE_DEFAULT << 3,
                    None,
                );
                DataReaderKind::LZMA2(decoder)
            }
            _ => DataReaderKind::Uncompressed(file.take(len)),
        };
        Ok(Self {
            inner,
            unpacked_size,
            data_size: len,
        })
    }

    pub fn new_with_seek(header: &Header, mut pkg_file: R, entry: &Entry) -> std::io::Result<Self> {
        let head_size = header
            .total_size()
            .map_err(|_| io::Error::from(io::ErrorKind::FileTooLarge))?
            as u64;
        pkg_file.seek(io::SeekFrom::Start(head_size + entry.offset))?;
        Self::new(header, pkg_file, entry.size)
    }

    pub fn finish(self, source: &mut impl PackageSrcExt<R>) -> Result<(), Error> {
        source.restore_reader(self.into_inner())
    }

    pub fn into_inner(self) -> R {
        match self.inner {
            DataReaderKind::Uncompressed(file) => file.into_inner(),
            DataReaderKind::LZMA2(xz_decoder) => xz_decoder.into_inner().into_inner(),
        }
    }
}

impl<R: Read> Read for DataReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            DataReaderKind::Uncompressed(file) => file.read(buf),
            DataReaderKind::LZMA2(reader) => reader.read(buf),
        }
    }
}
/// Implements writer based on data flags
#[expect(clippy::large_enum_variant, reason = "too short-lived to use heap")]
pub enum DataWriter {
    Uncompressed(File),
    LZMA2(lzma_rust2::Lzma2Writer<File>),
}

impl Write for DataWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Uncompressed(file) => file.write(buf),
            Self::LZMA2(xz_encoder) => xz_encoder.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Uncompressed(file) => file.flush(),
            Self::LZMA2(xz_encoder) => xz_encoder.flush(),
        }
    }
}

impl DataWriter {
    pub fn new(header: Packaging, mut file: File, len: u64) -> std::io::Result<Self> {
        let writer = match header {
            Packaging::LZMA2 => {
                file.write_all(&len.to_le_bytes())?;
                Self::LZMA2(lzma_rust2::Lzma2Writer::new(
                    file,
                    lzma_rust2::Lzma2Options::with_preset(5),
                ))
            }
            _ => Self::Uncompressed(file),
        };
        Ok(writer)
    }

    pub fn finish(self) -> std::io::Result<File> {
        match self {
            Self::Uncompressed(file) => Ok(file),
            Self::LZMA2(xz_encoder) => xz_encoder.finish(),
        }
    }
}
