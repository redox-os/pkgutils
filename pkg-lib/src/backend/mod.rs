pub mod pkgar_backend;
pub mod tar;

use std::io;
use thiserror::Error;

use crate::net_backend::{Callback, DownloadError};

// todo: make this better
#[derive(Error, Debug)]
pub enum Error {
    #[error("Please add repos")]
    ValidRepoNotFound,
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

pub trait Backend {
    fn install(&mut self, package: String) -> Result<(), Error>;
    fn uninstall(&mut self, package: String) -> Result<(), Error>;
    fn upgrade(&mut self, package: String) -> Result<(), Error>;
    fn get_installed_packages(&self) -> Result<Vec<String>, Error>;
}
