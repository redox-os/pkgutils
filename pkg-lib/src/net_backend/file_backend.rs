use std::{
    cell::RefCell,
    fs::File,
    io::{Read, Write},
    rc::Rc,
};

use crate::net_backend::DownloadBackendWriter;

use super::{Callback, DownloadBackend, DownloadError};

/// Local backend
#[derive(Clone)]
pub struct FileBackend {}

impl DownloadBackend for FileBackend {
    fn new() -> Result<Self, DownloadError> {
        Ok(Self {})
    }

    fn download(
        &self,
        remote_path: &str,
        writer: &mut DownloadBackendWriter,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut callback = callback.borrow_mut();
        let mut input = File::open(remote_path)?;
        let len = input.metadata()?.len();

        callback.start_download(len, remote_path);

        let mut data = [0; 8192];
        loop {
            let count = input.read(&mut data)?;
            if count == 0 {
                break;
            }

            writer.write_all(&data[..count])?;

            callback.increment_downloaded(count as u64);
        }
        writer.flush()?;

        callback.end_download();

        Ok(())
    }
}
