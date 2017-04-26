use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::error::Error;
use std::time::Duration;

use hyper::status::StatusCode;
use hyper::Client;
use hyper::net::HttpsConnector;
use hyper_rustls::TlsClient;
use hyper::error::Error as HyperError;
use hyper::header::ContentLength;

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let mut client = Client::with_connector(HttpsConnector::new(TlsClient::new()));
    client.set_read_timeout(Some(Duration::new(5, 0)));
    client.set_write_timeout(Some(Duration::new(5, 0)));
    let mut res = match client.get(remote_path).send() {
        Ok(res) => res,
        Err(HyperError::Io(err)) => return Err(err),
        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err.description()))
    };

    match res.status {
        StatusCode::Ok => {
            let mut count = 0;
            let length = res.headers.get::<ContentLength>().map_or(0, |h| h.0 as usize);

            let mut status = [b' '; 50];

            let mut file = File::create(&local_path)?;
            loop {
                let (percent, cols) = if count >= length {
                    (100, status.len())
                } else {
                    ((100 * count) / length, (status.len() * count) / length)
                };

                let _ = write!(stderr, "\r* {:>3}%[", percent);

                for i in 0..cols {
                    status[i] = b'=';
                }
                if cols < status.len() {
                    status[cols] = b'>';
                }

                let _ = stderr.write(&status);
                let _ = write!(stderr, "]");

                let mut buf = [0; 8192];
                let res = res.read(&mut buf)?;
                if res == 0 {
                    break;
                }
                count += file.write(&buf[.. res])?;
            }
            let _ = write!(stderr, "\n");

            Ok(())
        },
        _ => {
            let _ = write!(stderr, "* Failure {}\n", res.status);

            Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", remote_path)))
        }
    }
}
