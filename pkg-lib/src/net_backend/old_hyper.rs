use std::{path::Path, fs::File, io::{Write, self, Read}, time::Duration};


use hyper::{Client, net::HttpsConnector, status::StatusCode, header::ContentLength, Error};
use hyper_rustls::TlsClient;

use super::{DownloadBackend, DownloadError, Callback};



#[derive(Clone, Copy)]
pub struct HyperBackend {}

impl DownloadBackend for HyperBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        let mut client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
        client.set_read_timeout(Some(Duration::new(5, 0)));
        client.set_write_timeout(Some(Duration::new(5, 0)));
        let mut response = match client.get(remote_path).send() {
            Ok(response) => response,
            Err(Error::Io(err)) => return Err(err.into()),
            Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err).into()),
        };

        match response.status {
            StatusCode::Ok => {
                let mut count = 0;
                let length = response
                    .headers
                    .get::<ContentLength>()
                    .map_or(0, |h| h.0 as usize);

                let mut file = File::create(&local_path)?;
                
                callback.start(length as u64, remote_path);

                loop {
                    let mut buf = [0; 8192];
                    let res = response.read(&mut buf)?;
                    if res == 0 {
                        break;
                    }
                    count += file.write(&buf[..res])?;
                    callback.update(count);
                }

                callback.end();
                file.sync_all()?;
            }
            _ => {}
        }

        Ok(())
    }
}

