use std::{cell::RefCell, io, path::Path, rc::Rc};

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
    //#[error("Reqwest backend faild")]
    //Reqwest(#[from] reqwest::Error),
    #[error("IO error")]
    IO(#[from] io::Error),
}
