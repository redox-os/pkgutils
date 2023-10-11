use super::{DownloadBackend, DownloadError};
use crate::net_backend::Callback;
use reqwest::Client;
use std::{fs::File, os::unix::prelude::FileExt, path::Path};

#[derive(Clone, Copy)]
pub struct ReqwestBackend {}

impl DownloadBackend for ReqwestBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        self.async_download_reqwest(remote_path, local_path, callback)
    }
}

impl ReqwestBackend {
    #[tokio::main]
    async fn async_download_reqwest(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        let mut res = Client::builder()
            //.use_rustls_tls()
            .build()?
            .get(remote_path)
            .send()
            .await?;

        let output = File::create(local_path)?;
        let mut offset = 0;
        let len = res.content_length();

        callback.start(len.unwrap_or_default(), remote_path);

        while let Some(chunk) = res.chunk().await? {
            let chunk_len = chunk.len();

            callback.update(offset + chunk_len);

            output.write_at(&chunk, offset as u64)?;
            offset += chunk_len;
        }

        callback.end();

        Ok(())
    }
}
