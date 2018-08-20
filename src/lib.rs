#![deny(warnings)]

extern crate libflate;
extern crate hyper;
extern crate hyper_rustls;
extern crate octavo;
#[macro_use]
extern crate serde_derive;
extern crate tar;
extern crate toml;
extern crate pbr;
extern crate petgraph;
extern crate bidir_map;
extern crate ordermap;
#[macro_use] extern crate failure;

use libflate::gzip::Encoder;
use octavo::octavo_digest::Digest;
use octavo::octavo_digest::sha3::Sha512;
use std::str;
use std::fs::{self, File};
use std::io::{self, stderr, Read, Write, BufWriter};
use std::path::Path;
use download::DownloadError;
use database::DatabaseError;
use std::boxed::Box;
use packagemeta::PackageMetaError;

pub use download::download;
pub use packagemeta::{PackageMeta, PackageMetaList};
pub use package::{Package,PackageError};
pub use database::{Database, PackageDepends};

mod download;
mod packagemeta;
mod package;
mod database;

#[derive(Debug)]
pub struct Repo {
    local: String,
    remotes: Vec<String>,
    target: String,
}

#[derive(Debug,Fail)]
pub enum RepoError {
    #[fail(display="Download error: {}", _0)]
    DownloadError(DownloadError),
    #[fail(display="Error preforming critical I/O: {}", _0)]
    IoError(io::Error),
    #[fail(display="Database error: {}", _0)]
    DatabaseError(Box<DatabaseError>),
    #[fail(display="Package error: {}", _0)]
    PackageError(Box<PackageError>),
    #[fail(display="Package metadata gathering error: {}", _0)]
    PackageMetaError(PackageMetaError),

}

impl From<io::Error> for RepoError {
    fn from(err: io::Error) -> RepoError {
        RepoError::IoError(err)
    }
}
impl From<DownloadError> for RepoError {
    fn from(err: DownloadError) -> RepoError {
        RepoError::DownloadError(err)
    }
}
impl From<DatabaseError> for RepoError {
    fn from(err: DatabaseError) -> RepoError {
        RepoError::DatabaseError(Box::new(err))
    }
}
impl From<PackageError> for RepoError {
    fn from(err: PackageError) -> RepoError {
        RepoError::PackageError(Box::new(err))
    }
}
impl From<PackageMetaError> for RepoError {
    fn from(err: PackageMetaError) -> RepoError {
        RepoError::PackageMetaError(err)
    }
}

impl Repo {
    pub fn new(target: &str) -> Repo {
        let mut remotes = vec![];

        //TODO: Cleanup
        // This will add every line in every file in /etc/pkg.d to the remotes,
        // provided it does not start with #
        {
            let mut entries = vec![];
            if let Ok(read_dir) = fs::read_dir("/etc/pkg.d") {
                for entry_res in read_dir {
                    if let Ok(entry) = entry_res {
                        let path = entry.path();
                        if path.is_file() {
                            entries.push(path);
                        }
                    }
                }
            }

            entries.sort();

            for entry in entries {
                if let Ok(mut file) = File::open(entry) {
                    let mut data = String::new();
                    if let Ok(_) = file.read_to_string(&mut data) {
                        for line in data.lines() {
                            if ! line.starts_with('#') {
                                remotes.push(line.to_string());
                            }
                        }
                    }
                }
            }
        }

        Repo {
            local: format!("/tmp/pkg"),
            remotes: remotes,
            target: target.to_string()
        }
    }

    pub fn sync(&self, file: &str) -> Result<String,RepoError> {
        let local_path = format!("{}/{}", self.local, file);

        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut res = Err(DownloadError::IoError(io::Error::new(io::ErrorKind::NotFound, format!("no remote paths"))));
        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}/{}", remote, self.target, file);
            res = download(&remote_path, &local_path).map(|_| local_path.clone());
            if res.is_ok() {
                break;
            }
        }
        Ok(res?)
    }

    pub fn signature(&self, file: &str) -> Result<String,RepoError> {
        let mut data = vec![];
        File::open(&file)?.read_to_end(&mut data)?;

        let mut output = vec![0; Sha512::output_bytes()];
        let mut hash = Sha512::default();
        hash.update(&data);
        hash.result(&mut output);

        let mut encoded = String::new();
        for b in output.iter() {
            encoded.push_str(&format!("{:X}", b));
        }

        Ok(encoded)
    }

    pub fn clean(&self, package: &str) -> Result<String,RepoError> {
        let tardir = format!("{}/{}", self.local, package);
        fs::remove_dir_all(&tardir)?;
        Ok(tardir)
    }

    pub fn create(&self, package: &str) -> Result<String,RepoError> {
        if ! Path::new(package).is_dir() {
            return Err(RepoError::IoError(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", package))));
        }

        let sigfile = format!("{}.sig", package);
        let tarfile = format!("{}.tar.gz", package);

        {
            let file = File::create(&tarfile)?;
            let encoder = Encoder::new(BufWriter::new(file))?;

            let mut tar = tar::Builder::new(encoder);
            tar.follow_symlinks(false);
            tar.append_dir_all("", package)?;

            let encoder = tar.into_inner()?;
            let mut file = encoder.finish().into_result()?;
            file.flush()?;
        }

        let mut signature = self.signature(&tarfile)?;
        signature.push('\n');

        File::create(&sigfile)?.write_all(&signature.as_bytes())?;

        Ok(tarfile)
    }

    pub fn fetch_meta(&self, package: &str) -> Result<PackageMeta,RepoError> {
        let tomlfile = self.sync(&format!("{}.toml", package))?;

        let mut toml = String::new();
        File::open(tomlfile)?.read_to_string(&mut toml)?;

/*        PackageMeta::from_toml(&toml).map_err(|err| {
            RepoError::IoError(io::Error::new(io::ErrorKind::InvalidData, format!("TOML error: {}", err)))
        })*/
        Ok(PackageMeta::from_toml(&toml)?)
    }

    pub fn fetch(&self, package: &str) -> Result<Package,RepoError> {
        let sigfile = self.sync(&format!("{}.sig", package))?;

        let mut expected = String::new();
        File::open(sigfile)?.read_to_string(&mut expected)?;
        let expected = expected.trim();

        {
            let tarfile = format!("{}/{}.tar.gz", self.local, package);
            if let Ok(signature) = self.signature(&tarfile) {
                if signature == expected {
                    write!(stderr(), "* Already downloaded {}\n", package)?;
                    return Ok(Package::from_path(tarfile)?);
                }
            }
        }

        let tarfile = self.sync(&format!("{}.tar.gz", package))?;

        if self.signature(&tarfile)? != expected  {
            return Err(RepoError::IoError(io::Error::new(io::ErrorKind::InvalidData, format!("{} The signature given was not valid", package))));
        }

        Ok(Package::from_path(tarfile)?)
    }

    pub fn extract(&self, package: &str) -> Result<String,RepoError> {
        let tardir = format!("{}/{}", self.local, package);
        fs::create_dir_all(&tardir)?;
        self.fetch(package)?.install(&tardir)?;
        Ok(tardir)
    }

    pub fn add_remote(&mut self, remote: &str) {
        self.remotes.push(remote.to_string());
    }
}
