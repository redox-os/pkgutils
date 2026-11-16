use std::{
    borrow::Cow,
    io::{Read, Result as IoResult, Seek, SeekFrom},
    path::Path,
    sync::OnceLock,
};

use crate::{Error, PackageFile, ext::PackageSrcExt};
use pkgar_core::{Header, PackageSrc, PublicKey};
use pkgar_keys::PublicKeyFile;
use pkgar_repo::{PackageUrl, PublicKeyRemote};

pub(crate) fn open_or_download_pubkey(pkey_path: impl AsRef<Path>) -> Result<PublicKeyFile, Error> {
    Ok(PublicKeyRemote::download_or_read(
        &pkgar_repo::new_client(),
        pkey_path.as_ref(),
    )?)
}

static REPO_CLIENT: OnceLock<pkgar_repo::Agent> = OnceLock::new();

/// A package that is either opened from local or remote
pub enum GenericPackage {
    File(PackageFile),
    Url(PackageUrl<'static>),
}

impl GenericPackage {
    pub fn open_or_download(
        archive_path: impl AsRef<Path>,
        pkey: &PublicKey,
    ) -> Result<Self, Error> {
        let archive_path = archive_path.as_ref();

        {
            if let Some(archive_path_str) = archive_path.to_str()
                && pkgar_repo::is_remote(archive_path_str)
            {
                let agent = REPO_CLIENT.get_or_init(pkgar_repo::new_client);
                let pkg = PackageUrl::new(agent, archive_path_str.to_string(), pkey)?;
                return Ok(Self::Url(pkg));
            }
        }

        Ok(Self::File(PackageFile::new(archive_path, pkey)?))
    }

    pub fn split(
        &mut self,
        head_path: impl AsRef<Path>,
        data_path_opt: Option<impl AsRef<Path>>,
    ) -> Result<(), Error> {
        let data_offset = self.header().total_size()? as u64;
        let mut src = self.take_reader()?;

        src = PackageFile::split_inner(data_path_opt, head_path, data_offset, src)?;

        self.restore_reader(src)?;

        Ok(())
    }
    pub fn verify(&mut self, base_dir: impl AsRef<Path>) -> Result<(), Error> {
        let base_dir = base_dir.as_ref();
        let entries = self.read_entries()?;
        let mut pkg_file = self.take_reader()?;
        let header = self.header();

        PackageFile::verify_inner(
            Path::new(self.path().as_ref()),
            base_dir,
            entries,
            &mut pkg_file,
            header,
        )?;

        self.restore_reader(pkg_file)?;

        Ok(())
    }
}

impl PackageSrc for GenericPackage {
    type Err = Error;

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Self::Err> {
        match self {
            GenericPackage::File(package_file) => package_file.read_at(offset, buf),
            GenericPackage::Url(package_url) => {
                package_url.read_at(offset, buf).map_err(Error::from)
            }
        }
    }

    fn header(&self) -> Header {
        match self {
            GenericPackage::File(package_file) => package_file.header(),
            GenericPackage::Url(package_url) => package_url.header(),
        }
    }
}

impl PackageSrcExt<GenericReader> for GenericPackage {
    fn path(&self) -> Cow<'_, str> {
        match self {
            GenericPackage::File(package_file) => package_file.path(),
            GenericPackage::Url(package_url) => package_url.url().into(),
        }
    }

    fn take_reader(&mut self) -> Result<GenericReader, Error> {
        match self {
            GenericPackage::File(package_file) => {
                package_file.take_reader().map(GenericReader::File)
            }
            GenericPackage::Url(package_url) => {
                Ok(GenericReader::Url(package_url.take_reader().unwrap()))
            }
        }
    }

    fn restore_reader(&mut self, reader: GenericReader) -> Result<(), Error> {
        match (self, reader) {
            (GenericPackage::File(package_file), GenericReader::File(reader)) => {
                package_file.restore_reader(reader)
            }
            (GenericPackage::Url(package_url), GenericReader::Url(reader)) => {
                Ok(package_url.restore_reader(reader).unwrap())
            }
            _ => Err(Error::DataNotInitialized),
        }
    }
}

/// A reader that is either holding a file descriptor or remote cache
pub enum GenericReader {
    File(std::fs::File),
    Url(pkgar_repo::PackageUrlCache<'static>),
}

impl Read for GenericReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self {
            Self::File(f) => f.read(buf),
            Self::Url(u) => u.read(buf),
        }
    }
}

impl Seek for GenericReader {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        match self {
            Self::File(f) => f.seek(pos),
            Self::Url(u) => u.seek(pos),
        }
    }
}
