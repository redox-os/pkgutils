use std::{
    cell::RefCell,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    rc::Rc,
    time::Duration,
};

use hyper::{header::ContentLength, net::HttpsConnector, status::StatusCode, Client, Error};
use hyper_rustls::TlsClient;

use super::{Callback, DownloadBackend, DownloadError};

#[derive(Clone, Copy)]
pub struct HyperBackend {}

impl DownloadBackend for HyperBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<(), DownloadError> {
        let mut client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
        client.set_read_timeout(Some(Duration::new(5, 0)));
        client.set_write_timeout(Some(Duration::new(5, 0)));
        let mut response = match client.get(remote_path).send() {
            Ok(response) => response,
            Err(Error::Io(err)) => return Err(err.into()),
            Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err).into()),
        };

        let mut callback = callback.borrow_mut();
        if let StatusCode::Ok = response.status {
            let length = response
                .headers
                .get::<ContentLength>()
                .map_or(0, |h| h.0 as usize);
            let mut file = File::create(local_path)?;

            callback.start_download(length as u64, remote_path);
            loop {
                let mut buf = [0; 8192];
                let res = response.read(&mut buf)?;
                if res == 0 {
                    break;
                }
                let new_bytes = file.write(&buf[..res])?;
                callback.increment_downloaded(new_bytes);
            }
            callback.end_download();
            file.sync_all()?;
        }

        Ok(())
    }
}
