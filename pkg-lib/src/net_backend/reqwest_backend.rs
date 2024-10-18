use std::{
    cell::RefCell,
    fs::File,
    io::{Read, Write},
    path::Path,
    rc::Rc,
    time::Duration,
};

use super::{Callback, DownloadBackend, DownloadError};

#[derive(Clone)]
pub struct ReqwestBackend {
    client: reqwest::blocking::Client,
}

impl DownloadBackend for ReqwestBackend {
    fn new() -> Result<Self, DownloadError> {
        let client = reqwest::blocking::Client::builder()
            .brotli(true)
            .timeout(Duration::new(5, 0))
            .build()?;
        Ok(Self { client })
    }

    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut callback = callback.borrow_mut();

        let mut resp = self.client.get(remote_path).send()?;

        let len: u64 = resp.content_length().unwrap_or(0);

        let mut output = File::create(local_path)?;

        callback.start_download(len, remote_path);

        let mut data = [0; 8192];
        loop {
            let count = resp.read(&mut data)?;
            output.write(&data[..count])?;
            if count == 0 {
                break;
            }
            callback.increment_downloaded(count);
        }
        output.flush()?;

        callback.end_download();

        Ok(())
    }
}
