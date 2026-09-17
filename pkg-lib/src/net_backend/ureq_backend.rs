use std::{
    cell::RefCell,
    io::{Read, Write},
    rc::Rc,
    time::Duration,
};

use super::{Callback, DownloadBackend, DownloadError};
use crate::net_backend::DownloadBackendWriter;
use ureq::Agent;

/// Network backend
#[derive(Clone)]
pub struct UreqBackend {
    client: Agent,
}

impl DownloadBackend for UreqBackend {
    fn new() -> Result<Self, DownloadError> {
        let client = Agent::new_with_config(
            Agent::config_builder()
                .timeout_connect(Some(Duration::new(5, 0)))
                .build(),
        );
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
        let mut resp = self.client.get(remote_path).call()?;

        callback.download_start(remote_len.unwrap_or(0), remote_path);
        let mut resp = resp.body_mut().as_reader();
        let mut data = [0; 8192];

        loop {
            let count = resp.read(&mut data)?;
            writer.write_all(&data[..count])?;
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
