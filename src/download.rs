use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::error::Error;

use hyper::status::StatusCode;
use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_rustls::TlsClient;
use hyper::error::Error as HyperError;

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    write!(stderr(), "* Requesting {}\n", remote_path)?;

    let client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
    let mut res = match client.get(remote_path).send() {
        Ok(res) => res,
        Err(HyperError::Io(err)) => return Err(err),
        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err.description()))
    };

    let mut data = Vec::new();
    res.read_to_end(&mut data)?;

    match res.status {
        StatusCode::Ok => {
            write!(stderr(), "* Success {}\n", res.status)?;

            File::create(&local_path)?.write(data.as_slice())?;
            Ok(())
        },
        _ => {
            write!(stderr(), "* Failure {}\n", res.status)?;

            Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", remote_path)))
        }
    }
}
