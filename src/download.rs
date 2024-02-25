use std::fs::File;
use std::io::{self, stderr, Write};
use std::str::FromStr;

use hyper::client::{Client, HttpConnector};
use hyper::header::CONTENT_LENGTH;
use hyper::rt::{Future, Stream};  
use hyper::{Body, StatusCode};
use hyper::Uri;
use hyper_rustls::HttpsConnector;

use pbr::{ProgressBar, Units};

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let https = HttpsConnector::new(1);
    let client: Client<HttpsConnector<HttpConnector>, Body> = Client::builder().build(https);
    let uri = Uri::from_str(remote_path).expect("invalid uri");
    let response = match client.get(uri).wait() {
        Ok(response) => response,
        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
    };

    match response.status() {
        StatusCode::OK => {
            let mut count = 0;
            let length = response
                .headers()
                .get(CONTENT_LENGTH)
                .map_or(0, |_h| 0 as usize);

            let mut file = File::create(&local_path)?;
            let mut pb = ProgressBar::new(length as u64);
            pb.set_units(Units::Bytes);
            let body = response.into_body();
            body.for_each(|chunk| {
                let a =  file.write_all(&chunk)
                .map_err(|e| panic!("error={}", e));
                count += chunk.len();
                pb.set(count as u64);
               a
            }).wait().expect("failed");

            let _ = write!(stderr, "\n");

            file.sync_all()?;

            Ok(())
        }
        _ => {
            let _ = write!(stderr, "* Failure {}\n", response.status());

            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} not found", remote_path),
            ))
        }
    }
}
