use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use bytemuck::Zeroable;
use pkgar_core::{Header, PackageSrc, PublicKey};

use crate::ext::{DataReader, EntryExt, PackageSrcExt, copy_and_hash};
use crate::{Error, READ_WRITE_HASH_BUF_SIZE, wrap_io_err};

#[derive(Debug)]
pub struct PackageFile {
    path: PathBuf,
    src: Option<BufReader<File>>,
    header: Header,
}

impl PackageFile {
    pub fn new(path: impl AsRef<Path>, public_key: &PublicKey) -> Result<PackageFile, Error> {
        let path = path.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(wrap_io_err!(path.clone(), "Opening pkgar file"))?;

        let mut new = PackageFile {
            path,
            src: Some(BufReader::new(file)),

            // Need a blank header to construct the PackageFile, since we need to
            //   use a method of PackageSrc in order to get the actual header...
            header: Header::zeroed(),
        };

        new.header = new.read_header(public_key)?;
        Ok(new)
    }

    pub fn split(
        &mut self,
        head_path: impl AsRef<Path>,
        data_path_opt: Option<impl AsRef<Path>>,
    ) -> Result<(), Error> {
        let data_offset = self.header().total_size()? as u64;
        let mut src = self.take_reader()?;

        src = Self::split_inner(data_path_opt, head_path, data_offset, src)?;

        self.restore_reader(src)
    }

    pub(crate) fn split_inner<R>(
        data_path_opt: Option<impl AsRef<Path>>,
        head_path: impl AsRef<Path>,
        data_offset: u64,
        mut src: R,
    ) -> Result<R, Error>
    where
        R: Read + Seek,
    {
        let head_path = head_path.as_ref();
        if let Some(data_path) = data_path_opt {
            let data_path = data_path.as_ref();
            let mut data_file =
                fs::File::create(data_path).map_err(wrap_io_err!(data_path, "Opening data"))?;
            src.seek(SeekFrom::Start(data_offset))
                .map_err(wrap_io_err!(data_path, "Seeking data"))?;
            std::io::copy(&mut src, &mut data_file)
                .map_err(wrap_io_err!(data_path, "Writing data"))?;
        }
        {
            let mut head_file =
                fs::File::create(head_path).map_err(wrap_io_err!(head_path, "Opening head"))?;
            src.seek(SeekFrom::Start(0))
                .map_err(wrap_io_err!(head_path, "Seeking head"))?;
            let mut src_taken = src.take(data_offset);
            std::io::copy(&mut src_taken, &mut head_file)
                .map_err(wrap_io_err!(head_path, "Writing head"))?;
            Ok(src_taken.into_inner())
        }
    }

    pub fn verify(&mut self, base_dir: impl AsRef<Path>) -> Result<(), Error> {
        let base_dir = base_dir.as_ref();
        let entries = self.read_entries()?;
        let mut pkg_file = self.take_reader()?;
        let header = self.header();

        pkg_file = Self::verify_inner(&self.path, base_dir, entries, pkg_file, header)?;

        self.restore_reader(pkg_file)
    }

    pub(crate) fn verify_inner<R>(
        pkg_path: &Path,
        base_dir: &Path,
        entries: Vec<pkgar_core::Entry>,
        mut pkg_file: R,
        header: Header,
    ) -> Result<R, Error>
    where
        R: Read + Seek,
    {
        let mut buf = vec![0; READ_WRITE_HASH_BUF_SIZE];
        for entry in entries {
            let expected_path = base_dir.join(entry.check_path()?);

            let mut expected =
                File::open(&expected_path).map_err(wrap_io_err!(expected_path, "Opening file"))?;

            let (count, hash) = copy_and_hash(&mut expected, &mut std::io::sink(), &mut buf)
                .map_err(wrap_io_err!(expected_path, "Writing file to to black hole"))?;

            let reader = DataReader::new_with_seek(&header, pkg_file, &entry)
                .map_err(wrap_io_err!(pkg_path, "Reading pkg data"))?;
            entry.verify(hash, count, &reader)?;
            pkg_file = reader.into_inner();
        }

        Ok(pkg_file)
    }
}

impl PackageSrc for PackageFile {
    type Err = Error;

    fn header(&self) -> Header {
        self.header
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Self::Err> {
        let Some(src) = &mut self.src else {
            return Err(Error::DataNotInitialized);
        };
        src.seek(SeekFrom::Start(offset))
            .map_err(wrap_io_err!("Seek at read_at"))?;
        src.read_exact(buf)
            .map_err(wrap_io_err!("Read at read_at"))?;
        Ok(buf.len())
    }
}

impl PackageSrcExt<File> for PackageFile {
    fn path(&self) -> std::borrow::Cow<'_, str> {
        self.path.to_string_lossy()
    }

    fn take_reader(&mut self) -> Result<File, Error> {
        match self.src.take() {
            Some(reader) => Ok(reader.into_inner()),
            None => Err(Error::DataNotInitialized),
        }
    }

    fn restore_reader(&mut self, reader: File) -> Result<(), Error> {
        match self.src {
            Some(_) => Err(Error::Core(pkgar_core::Error::NotSupported)),
            ref mut src => {
                *src = Some(BufReader::new(reader));
                Ok(())
            }
        }
    }
}
