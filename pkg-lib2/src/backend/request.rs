use std::{fs::File, os::unix::prelude::FileExt};
use reqwest::Client;
use crate::Callback;
use super::{DownloadBackend, DownloadError};

#[derive(Clone, Copy)]
pub struct ReqwestBackend {}

impl DownloadBackend for ReqwestBackend {
    fn download(&self, remote_path: &str, local_path: &str) -> Result<(), DownloadError> {
        self.download_reqwest(remote_path, local_path)
    }

    fn download_with_callback(
        &self,
        remote_path: &str,
        local_path: &str,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        self.download_with_callback_reqwest(remote_path, local_path, callback)
    }
}

impl ReqwestBackend {
    #[tokio::main]
    async fn download_with_callback_reqwest(
        &self,
        remote_path: &str,
        local_path: &str,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        let mut res = Client::builder()
            .use_rustls_tls()
            .build()?
            .get(remote_path)
            .send()
            .await?;

        let output = File::create(local_path)?;
        let mut offset = 0;
        let len = res.content_length();

        callback.start(len.unwrap_or_default(), remote_path);

        while let Some(chunk) = res.chunk().await? {
            let chunk_len = chunk.len() as u64;

            callback.update(offset + chunk_len);

            output.write_at(&chunk, offset)?;
            offset += chunk_len;
        }

        callback.end();

        Ok(())
    }

    #[tokio::main]
    async fn download_reqwest(
        &self,
        remote_path: &str,
        local_path: &str,
    ) -> Result<(), DownloadError> {
        let mut res = Client::builder()
            .use_rustls_tls()
            .build()?
            .get(remote_path)
            .send()
            .await?;

        let output = File::create(local_path)?;
        let mut offset = 0;

        while let Some(chunk) = res.chunk().await? {
            let chunk_len = chunk.len() as u64;

            output.write_at(&chunk, offset)?;
            offset += chunk_len;
        }

        Ok(())
    }
}
