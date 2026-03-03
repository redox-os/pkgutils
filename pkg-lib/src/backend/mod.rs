pub mod pkgar_backend;

use std::io;
use thiserror::Error;

use crate::{
    net_backend::DownloadError,
    package::{PackageError, RemotePackage, Repository},
    PackageName, PackageState,
};

// todo: make this better
#[derive(Error, Debug)]
pub enum Error {
    #[error("Please add repos")]
    ValidRepoNotFound,
    #[error("Repository path is not valid: {0:?}")]
    RepoPathInvalid(String),
    #[error("Repository recursed infinitely with: {0:?}")]
    RepoRecursion(Vec<PackageName>),
    #[error("Cached package {0:?} source repo is not found")]
    RepoCacheNotFound(PackageName),
    #[error("Public key for {0:?} is not available")]
    RepoNotLoaded(String),
    #[error("Package {0:?} not found")]
    PackageNotFound(PackageName),
    #[error("Package {0:?} not installed")]
    PackageNotInstalled(PackageName),
    #[error("Package {0:?} name invalid")]
    PackageNameInvalid(String),
    #[error("{0}")]
    Package(#[from] PackageError),
    #[error("Path {0:?} isn't a Valid Unicode String")]
    PathIsNotValidUnicode(String),
    #[error("Content of {0:?} is not a valid UTF-8 content")]
    ContentIsNotValidUnicode(String),
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
    /// individually install a package
    fn install(&mut self, package: RemotePackage) -> Result<(), Error>;
    /// individually uninstall a package
    fn uninstall(&mut self, package: PackageName) -> Result<(), Error>;
    /// individually upgrade a package
    fn upgrade(&mut self, package: PackageName) -> Result<(), Error>;
    /// download package TOML data
    fn get_package_detail(&self, package: &PackageName) -> Result<RemotePackage, Error>;
    /// download repo TOML data
    fn get_repository_detail(&self) -> Result<Repository, Error>;
    /// get state of current installation
    fn get_package_state(&self) -> PackageState;
    /// check if there's pending transaction conflicts before committing
    fn commit_check_conflict(&self) -> Result<&Vec<pkgar::TransactionConflict>, Error>;
    /// commit all pending changes, and set state of current installation
    fn commit_state(&mut self, new_state: PackageState) -> Result<usize, Error>;
    /// abort all pending changes
    fn abort_state(&mut self) -> Result<usize, Error>;
}
