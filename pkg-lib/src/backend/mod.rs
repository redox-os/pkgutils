pub mod pkgar_backend;

use std::io;
use thiserror::Error;

use crate::{
    net_backend::DownloadError,
    package::{PackageError, Repository},
    Package, PackageName,
};

// todo: make this better
#[derive(Error, Debug)]
pub enum Error {
    #[error("Please add repos")]
    ValidRepoNotFound,
    #[error("Repository path is not valid")]
    RepoPathInvalid,
    #[error("Cached package {0:?} source repo is not found")]
    RepoCacheNotFound(PackageName),
    #[error("Package {0:?} not found")]
    PackageNotFound(PackageName),
    #[error("Package {0:?} name invalid")]
    PackageNameInvalid(String),
    #[error("{0}")]
    Package(#[from] PackageError),
    #[error("Path {0:?} isn't a Valid Unicode String")]
    PathIsNotValidUnicode(String),
    #[error("You don't have permissions required for this action, try performing it as root")]
    MissingPermissions,

    #[error("Package {0:?} is protected")]
    ProtectedPackage(PackageName),

    #[error("IO error: {0}")]
    IO(io::Error),
    #[error("Download error: {0}")]
    Download(#[from] DownloadError),
    #[error("Download error: {0}")]
    TomlRead(#[from] toml::de::Error),
    #[error("pkgar_keys error: {0}")]
    PkgarKeys(#[from] pkgar_keys::Error),
    #[error("pkgar error: {0}")]
    Pkgar(Box<pkgar::Error>),
    #[error("pkgar error: {0}")]
    PkgarAnyhow(#[from] anyhow::Error),
}

impl From<pkgar::Error> for Error {
    fn from(value: pkgar::Error) -> Self {
        Error::Pkgar(Box::new(value))
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        if value.kind() == std::io::ErrorKind::PermissionDenied {
            return Error::MissingPermissions;
        } else {
            return Error::IO(value);
        }
    }
}

pub trait Backend {
    fn install(&mut self, package: PackageName) -> Result<(), Error>;
    fn uninstall(&mut self, package: PackageName) -> Result<(), Error>;
    fn upgrade(&mut self, package: PackageName) -> Result<(), Error>;
    fn get_installed_packages(&self) -> Result<Vec<PackageName>, Error>;
    fn get_package_detail(&self, package: &PackageName) -> Result<Package, Error>;
    fn get_repository_detail(&self) -> Result<Repository, Error>;
}
