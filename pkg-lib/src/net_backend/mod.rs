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

// this feals wrong
#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
}
