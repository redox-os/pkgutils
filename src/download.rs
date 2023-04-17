use std::fs::File;
use std::io::{self, stderr, BufWriter, Read, Write};

use hyper::{Body, Client, Request, StatusCode};
use hyper_tls::HttpsConnector;

use pbr::{ProgressBar, Units};

pub async fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);
    let request = Request::builder()
        .uri(remote_path)
        .body(Body::empty())
        .unwrap();

    let response = match client.request(request).await {
        Ok(response) => response,
        Err(err) => {
            let _ = write!(stderr, "* Failure {}\n", err);

            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} not found", remote_path),
            ));
        }
    };

    match response.status() {
        StatusCode::OK => {
            let length = response
                .headers()
                .get("content-length")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.parse().ok())
                .unwrap_or(0);

            let mut path = match File::create(&local_path) {
                Ok(file) => file,
                Err(err) => {
                    let _ = write!(stderr, "* Failure {}\n", err);

                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("{} not found", remote_path),
                    ));
                }
            };
            let mut file = BufWriter::new(&mut path);
            let mut pb = ProgressBar::new(length as u64);
            pb.set_units(Units::Bytes);

            // to stream
            let body = hyper::body::to_bytes(response.into_body()).await;

            // to reader
            let mut reader = io::Cursor::new(body.unwrap());

            // write to file
            let mut buffer = [0; 1024];
            loop {
                let n = match reader.read(&mut buffer) {
                    Ok(n) if n > 0 => n,
                    _ => break,
                };
                file.write_all(&buffer[..n])?;
                pb.add(n as u64);
            }
            let _ = write!(stderr, "\n");

            file.flush()?;
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
