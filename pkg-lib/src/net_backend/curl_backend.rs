use std::{cell::RefCell, rc::Rc};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

use crate::callback::Callback;
use crate::net_backend::DownloadBackendWriter;

use super::{DownloadBackend, DownloadError};

/// Network backend using external curl
#[derive(Clone, Default)]
pub struct CurlBackend;

impl DownloadBackend for CurlBackend {
    fn new() -> Result<Self, DownloadError> {
        Ok(Self)
    }

    fn download(
        &self,
        remote_path: &str,
        remote_len: Option<u64>,
        writer: &mut DownloadBackendWriter,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut child = Command::new("curl")
            .arg("-sSL")
            .arg(remote_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout = child.stdout.take().ok_or_else(|| {
            DownloadError::IO(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        })?;

        let mut stderr = child.stderr.take().ok_or_else(|| {
            DownloadError::IO(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        })?;

        let mut callback = callback.borrow_mut();
        callback.download_start(remote_len.unwrap_or(0), remote_path);

        let mut data = [0; 8192];
        loop {
            let count = stdout.read(&mut data)?;

            if count == 0 {
                break;
            }

            writer.write_all(&data[..count])?;
            callback.download_increment(count as u64);
        }

        writer.flush()?;
        callback.download_end();

        let status = child.wait()?;

        if !status.success() {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf);
            return Err(DownloadError::Other(format!(
                "curl exit code {}:\n{}",
                status.code().unwrap_or(0),
                String::from_utf8_lossy(&buf)
            )));
        }

        Ok(())
    }
}
