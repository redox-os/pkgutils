use bytemuck::Zeroable;
use pkgar_core::{Entry, Header, PackageSrc, PublicKey};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::Error;
use crate::{ext::EntryExt, wrap_io_err};

#[derive(Debug)]
pub struct PackageHead {
    #[allow(dead_code)]
    head_path: PathBuf,
    root_path: PathBuf,
    pub(crate) src: BufReader<File>,
    header: Header,
}

impl PackageHead {
    pub fn new(
        head_path: impl AsRef<Path>,
        root_path: impl AsRef<Path>,
        public_key: &PublicKey,
    ) -> Result<PackageHead, Error> {
        let head_path = head_path.as_ref().to_path_buf();
        let root_path = root_path.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .read(true)
            .open(&head_path)
            .map_err(wrap_io_err!(head_path.clone(), "Open head"))?;

        let mut new = PackageHead {
            head_path,
            root_path,
            src: BufReader::new(file),

            // Need a blank header to construct the PackageHead, since we need to
            //   use a method of PackageSrc in order to get the actual header...
            header: Header::zeroed(),
        };

        new.header = new.read_header(public_key)?;
        Ok(new)
    }
}

impl PackageSrc for PackageHead {
    type Err = Error;

    fn header(&self) -> Header {
        self.header
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Self::Err> {
        self.src
            .seek(SeekFrom::Start(offset))
            .map_err(wrap_io_err!("Seek head"))?;
        self.src
            .read_exact(buf)
            .map_err(wrap_io_err!("Read head"))
            .map(|()| buf.len())
    }

    /// Read from this src at a given entry's data with a given offset within that entry
    fn read_entry(
        &mut self,
        entry: &Entry,
        offset: u64,
        buf: &mut [u8],
    ) -> Result<usize, Self::Err> {
        let end = Self::calculate_end(&entry, offset, buf)?;

        if end == 0 {
            return Ok(0);
        }

        let relative_path = entry.check_path()?;
        let entry_path = self.root_path.join(relative_path);
        let mut entry_file = OpenOptions::new()
            .read(true)
            .open(&entry_path)
            .map_err(wrap_io_err!(entry_path.clone(), "Open entry"))?;

        entry_file
            .seek(SeekFrom::Start(offset))
            .map_err(wrap_io_err!(entry_path.clone(), "Seek entry"))?;

        entry_file
            .read(&mut buf[..end])
            .map_err(wrap_io_err!(entry_path.clone(), "Read entry"))
    }
}
