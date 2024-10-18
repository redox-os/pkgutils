use std::{cell::RefCell, io, path::Path, rc::Rc};

use thiserror::Error;

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

// why is callback here
pub trait Callback {
    fn start_download(&mut self, length: u64, file: &str);
    fn increment_downloaded(&mut self, downloaded: usize);
    fn end_download(&mut self);

    fn conflict(&mut self) {}

    // todo: add error handeling
    fn error(&mut self) {}
}

// this feals wrong
#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IO(#[from] io::Error),
}
