use std::{
    cell::RefCell,
    io::{Read, Write},
    rc::Rc,
    time::Duration,
};

use super::{Callback, DownloadBackend, DownloadError};
use crate::net_backend::DownloadBackendWriter;
use reqwest::blocking::Client;

/// Network backend
#[derive(Clone)]
pub struct ReqwestBackend {
    client: Client,
}

impl DownloadBackend for ReqwestBackend {
    fn new() -> Result<Self, DownloadError> {
        let client = Client::builder()
            .brotli(true)
            .connect_timeout(Duration::new(5, 0))
            .build()?;
        Ok(Self { client })
    }

    fn download(
        &self,
        remote_path: &str,
        remote_len: Option<u64>,
        writer: &mut DownloadBackendWriter,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut callback = callback.borrow_mut();

        let mut resp = self.client.get(remote_path).send()?.error_for_status()?;

        let len: u64 = resp.content_length().unwrap_or(remote_len.unwrap_or(0));

        callback.download_start(len, remote_path);

        let mut data = [0; 8192];
        loop {
            let count = resp.read(&mut data)?;
            writer.write(&data[..count])?;
            if count == 0 {
                break;
            }
            callback.download_increment(count as u64);
        }
        writer.flush()?;

        callback.download_end();

        Ok(())
    }
}
