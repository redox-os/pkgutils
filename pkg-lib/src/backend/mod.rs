pub mod pkgar_backend;
pub mod tar;
pub mod reqwest_backend;
pub mod hyper_backend;
pub mod ureq_backend;

use std::{io, path::Path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Please add repos")]
    NoReposWereAdded,
    #[error("Package not found")]
    PackageNotFound(String),
    #[error("Path isn't a Valid Unicode String")]
    PathIsNotValidUnitcode(String),

    #[error("Package is protected")]
    ProtectedPackage(String),

    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Download error")]
    Download(#[from] DownloadError),
    #[error("Download error")]
    TomlRead(#[from] toml::de::Error),
    #[error("pkgar_keys error")]
    PkgarKeys(#[from] pkgar_keys::Error),
    #[error("pkgar error")]
    Pkgar(Box<pkgar::Error>),
}

impl From<pkgar::Error> for Error {
    fn from(value: pkgar::Error) -> Self {
        Error::Pkgar(Box::new(value))
    }
}

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Reqwest backend faild")]
    Reqwest(#[from] reqwest::Error),
    #[error("IO error")]
    IO(#[from] io::Error),
}

pub trait Callback {
    fn start(&mut self, length: u64, file: &str);
    // todo: change to increment
    fn update(&mut self, downloaded: usize);
    fn end(&mut self);
    // todo add error handeling
}

pub trait Backend {
    fn install(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error>;
    fn uninstall(&mut self, package: String) -> Result<(), Error>;
    fn upgrade(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error>;
    fn get_installed_packages(&self) -> Result<Vec<String>, Error>;
}

pub trait DownloadBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError>;
}
