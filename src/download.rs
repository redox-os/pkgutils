use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::time::Duration;

use hyper::status::StatusCode;
use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_rustls::TlsClient;
use hyper::error::Error as HyperError;
use hyper::header::ContentLength;

use pbr::{ProgressBar, Units};

#[derive(Debug,Fail)]
pub enum DownloadError {
    #[fail(display="Critical IO error: {}", _0)]
    IoError(io::Error),
    #[fail(display="We could not find your package: {}", _0)]
    NotFound(io::Error),
    #[fail(display="{}",_0)]
    HyperError(HyperError),
}

impl From<io::Error> for DownloadError {
    fn from(err: io::Error) -> DownloadError {
        DownloadError::IoError(err)
    }
}

impl From<HyperError> for DownloadError {
    fn from(err: HyperError) -> DownloadError {
        DownloadError::HyperError(err)
    }
}

pub fn download(remote_path: &str, local_path: &str) -> Result<(), DownloadError> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let mut client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
    client.set_read_timeout(Some(Duration::new(5, 0)));
    client.set_write_timeout(Some(Duration::new(5, 0)));
    let mut response = client.get(remote_path).send()?;

    match response.status {
        StatusCode::Ok => {
            let mut count = 0;
            let length = response.headers.get::<ContentLength>().map_or(0, |h| h.0 as usize);

            let mut file = File::create(&local_path)?;
            let mut pb = ProgressBar::new(length as u64);
            pb.set_units(Units::Bytes);
            loop {
                let mut buf = [0; 8192];
                let res = response.read(&mut buf)?;
                if res == 0 {
                    break;
                }
                count += file.write(&buf[.. res])?;
                pb.set(count as u64);
            }
            let _ = write!(stderr, "\n");

            file.sync_all()?;

            Ok(())
        },
        _ => {
            let _ = write!(stderr, "* Failure {}\n", response.status);

            Err(DownloadError::NotFound(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", remote_path))))
        }
    }
}
