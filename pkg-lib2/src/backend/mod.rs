use std::io;

use hyper::http::uri::InvalidUri;
use crate::{Callback, Error};

pub mod pkgar;
pub mod request;
pub mod tar;

#[derive(Debug)]
pub enum DownloadError {
    IO(io::Error),
    Reqwest(reqwest::Error),
    Hyper(hyper::Error),
    InvalidUri(InvalidUri),
    NoReposWereAdded,
}

impl From<io::Error> for DownloadError {
    fn from(value: io::Error) -> Self {
        DownloadError::IO(value)
    }
}
impl From<reqwest::Error> for DownloadError {
    fn from(value: reqwest::Error) -> Self {
        DownloadError::Reqwest(value)
    }
}
impl From<hyper::Error> for DownloadError {
    fn from(value: hyper::Error) -> Self {
        DownloadError::Hyper(value)
    }
}
impl From<InvalidUri> for DownloadError {
    fn from(value: InvalidUri) -> Self {
        DownloadError::InvalidUri(value)
    }
}

pub trait DownloadBackend {
    fn download(&self, remote_path: &str, local_path: &str) -> Result<(), DownloadError>;
    fn download_with_callback(
        &self,
        remote_path: &str,
        local_path: &str,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError>;
}

pub trait Backend {
    fn install(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error>;
    fn uninstall(&mut self, package: String) -> Result<(), Error>;
    fn upgrade(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error>;
    fn get_installed_packages(&self) -> Result<Vec<String>, Error>;
}
