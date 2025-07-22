use std::{cell::RefCell, io, path::Path, rc::Rc};
use thiserror::Error;

use crate::Callback;

mod reqwest_backend;

pub use reqwest_backend::ReqwestBackend as DefaultNetBackend;

pub trait DownloadBackend {
    fn new() -> Result<Self, DownloadError>
    where
        Self: Sized;

    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError>;

    fn file_size(&self) -> Option<usize> {
        None
    }
}

#[derive(Error, Debug)]
pub enum DownloadError {
    // Specific variant for timeout errors
    #[error("Download timed out")]
    Timeout,
    // Specific variant for HTTP status errors (e.g., 404, 500)
    #[error("HTTP error status: {0}")]
    HttpStatus(reqwest::StatusCode),
    // Fallback for other generic reqwest errors
    #[error("Other reqwest error: {0}")]
    Reqwest(reqwest::Error),
    // IO errors remain the same
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
}

impl From<reqwest::Error> for DownloadError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            DownloadError::Timeout
        } else if err.is_status() {
            DownloadError::HttpStatus(
                err.status()
                    .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
            )
        } else {
            DownloadError::Reqwest(err)
        }
    }
}
