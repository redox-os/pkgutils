use std::{path::Path, io};

use thiserror::Error;

//mod reqwest;
//mod ureq;
mod hyper;

//pub use reqwest::ReqwestBackend as DefaultNetBackend;
pub use hyper::HyperBackend as DefaultNetBackend;


pub trait DownloadBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError>;
}


pub trait Callback {
    fn start(&mut self, length: u64, file: &str);
    // todo: change to increment
    fn update(&mut self, downloaded: usize);
    fn end(&mut self);
    // todo add error handeling
}

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Reqwest backend faild")]
    Reqwest(#[from] reqwest::Error),
    #[error("IO error")]
    IO(#[from] io::Error),
}