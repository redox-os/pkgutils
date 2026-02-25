use std::{
    cell::RefCell,
    fs::File,
    io::{self, Write},
    path::Path,
    rc::Rc,
};
use thiserror::Error;

mod file_backend;
mod reqwest_backend;

use crate::callback::Callback;
pub use file_backend::FileBackend as DefaultLocalBackend;
pub use reqwest_backend::ReqwestBackend as DefaultNetBackend;

pub enum DownloadBackendWriter {
    ToFile(File),
    ToBuf(Vec<u8>),
}

impl Write for DownloadBackendWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            DownloadBackendWriter::ToFile(file) => file.write(buf),
            DownloadBackendWriter::ToBuf(items) => items.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            DownloadBackendWriter::ToFile(file) => file.flush(),
            DownloadBackendWriter::ToBuf(items) => items.flush(),
        }
    }
}

impl DownloadBackendWriter {
    pub fn to_inner_buf(self) -> Vec<u8> {
        match self {
            DownloadBackendWriter::ToBuf(items) => items,
            _ => panic!("Logic error, should be a buffer going here"),
        }
    }
    pub fn to_inner_file(self) -> File {
        match self {
            DownloadBackendWriter::ToFile(file) => file,
            _ => panic!("Logic error, should be a file handle going here"),
        }
    }
}

pub trait DownloadBackend {
    fn new() -> Result<Self, DownloadError>
    where
        Self: Sized;

    fn download(
        &self,
        remote_path: &str,
        writer: &mut DownloadBackendWriter,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError>;

    fn download_to_file(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut output = DownloadBackendWriter::ToFile(File::create(local_path)?);
        self.download(remote_path, &mut output, callback)
    }

    fn download_to_buf(
        &self,
        remote_path: &str,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Vec<u8>, DownloadError> {
        let mut output = DownloadBackendWriter::ToBuf(Vec::new());
        self.download(remote_path, &mut output, callback)?;
        Ok(output.to_inner_buf())
    }

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
