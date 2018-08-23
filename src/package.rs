use libflate::gzip::Decoder;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::io::{self, Error, ErrorKind, Read};
use tar::{Archive, EntryType};
use std::io::BufReader;
use RepoError;

use packagemeta::PackageMeta;

pub struct Package {
    archive: Archive<Decoder<BufReader<File>>>,
    path: PathBuf,
    meta: Option<PackageMeta>,
}


#[derive(Debug,Fail)]
pub enum PackageError {
    #[fail(display="Critical I/O error: {}", _0)]
    IoError(#[cause] io::Error),
    #[fail(display="{}", _0)]
    RepoError(RepoError),
    #[fail(display="Archive error: {}", _0)]
    ArchiveError(io::Error),
    #[fail(display="{}", _0)]
    MetadataNotFound(String),
}

//all io::Errors the ? macro is preformed on inside this file are related to archives
impl From<io::Error> for PackageError {
    fn from(err: io::Error) -> PackageError {
        PackageError::ArchiveError(err)
    }
}
impl From<RepoError> for PackageError {
    fn from(err: RepoError) -> PackageError {
        PackageError::RepoError(err)
    }
}

impl Package {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self,PackageError> {
        let file = File::open(&path)?;
        let decoder = Decoder::new(BufReader::new(file))?;

        let mut ar = Archive::new(decoder);
        ar.set_preserve_permissions(true);
        Ok(Package{archive: ar, path: path.as_ref().to_path_buf(), meta: None})
    }

    pub fn install(&mut self, dest: &str)-> Result<(),PackageError> {
        Ok(self.archive.unpack(dest)?)
    }

    pub fn list(&mut self) -> Result<(),PackageError> {
        for i in self.archive.entries()? {
            println!("{}", i?.path()?.display());
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn meta(&mut self) -> Result<&PackageMeta,PackageError> {
        if self.meta.is_none() {
            let mut toml = None;
            for entry in self.archive.entries()? {
                let mut entry = entry?;
                if entry.header().entry_type() != EntryType::Directory && entry.path()?.starts_with("pkg") {
                    if toml.is_none() {
                        let mut text = String::new();
                        entry.read_to_string(&mut text)?;
                        toml = Some(text);
                    } else {
                        return Err(PackageError::IoError(Error::new(ErrorKind::Other, "Multiple metadata files in package")));
                    }
                }
            }

            if let Some(toml) = toml {
                self.meta = PackageMeta::from_toml(&toml).ok();
            } else {
                return Err(PackageError::MetadataNotFound(String::from("Package metadata not found")));
            }
        }

        Ok(self.meta.as_ref().unwrap())
    }
}
