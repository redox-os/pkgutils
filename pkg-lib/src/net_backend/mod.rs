use std::{path::Path, io};

use thiserror::Error;

//mod reqwest;
//mod hyper;
//mod ureq;
mod old_hyper;

//pub use reqwest::ReqwestBackend as DefaultNetBackend;
//pub use hyper::HyperBackend as DefaultNetBackend;
//pub use ureq::UreqBackend as DefaultNetBackend;
pub use old_hyper::HyperBackend as DefaultNetBackend;

pub trait DownloadBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError>;
}

// why is callback here
pub trait Callback {
    fn start(&mut self, length: u64, file: &str);
    // change to increment
    fn update(&mut self, downloaded: usize);
    fn end(&mut self);
    // add error handeling
}

// this feals wrong
#[derive(Error, Debug)]
pub enum DownloadError {
    //#[error("Reqwest backend faild")]
    //Reqwest(#[from] reqwest::Error),
    #[error("IO error")]
    IO(#[from] io::Error),
}