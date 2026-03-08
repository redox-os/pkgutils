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
        _remote_len: Option<u64>,
        writer: &mut DownloadBackendWriter,
        // do not handle callback as curl has it's own progress bar
        _: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut child = Command::new("curl")
            .arg("-L")
            .arg("-#")
            .arg("-S")
            .arg(remote_path)
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdout = child.stdout.take().ok_or_else(|| {
            DownloadError::IO(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        })?;

        let mut data = [0; 8192];
        loop {
            let count = stdout.read(&mut data)?;

            if count == 0 {
                break;
            }

            writer.write_all(&data[..count])?;
        }

        writer.flush()?;

        let status = child.wait()?;

        if !status.success() {
            return Err(DownloadError::IO(std::io::Error::from(
                std::io::ErrorKind::NotFound,
            )));
        }

        Ok(())
    }
}
