use std::{
    cell::RefCell,
    fs::File,
    io::{Read, Write},
    path::Path,
    rc::Rc,
    time::Duration,
};

use super::{Callback, DownloadBackend, DownloadError};
use reqwest::blocking::Client;

#[derive(Clone)]
pub struct ReqwestBackend {
    client: Client,
    client_no_brotli: Client,
}

impl DownloadBackend for ReqwestBackend {
    fn new() -> Result<Self, DownloadError> {
        let client = Client::builder()
            .brotli(true)
            .connect_timeout(Duration::new(5, 0))
            .build()?;
        let client_no_brotli = Client::builder()
            .brotli(false)
            .connect_timeout(Duration::new(5, 0))
            .build()?;
        Ok(Self {
            client,
            client_no_brotli,
        })
    }

    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut callback = callback.borrow_mut();

        let mut resp = self.client.get(remote_path).send()?.error_for_status()?;

        let len: u64 = resp.content_length().unwrap_or_else(|| {
            self.client_no_brotli
                .head(remote_path)
                .send()
                .ok()
                .and_then(|resp_inner| {
                    resp_inner
                        .headers()
                        .get("content-length")
                        .and_then(|header| header.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                })
                .unwrap_or(0)
        });

        let mut output = File::create(local_path)?;

        callback.start_download(len, remote_path);

        let mut data = [0; 8192];
        loop {
            let count = resp.read(&mut data)?;
            output.write(&data[..count])?;
            if count == 0 {
                break;
            }
            callback.increment_downloaded(count as u64);
        }
        output.flush()?;

        callback.end_download();

        Ok(())
    }
}
