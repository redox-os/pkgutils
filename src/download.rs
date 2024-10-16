use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::time::Duration;

use reqwest::{
    blocking::Client,
    StatusCode,
};

use pbr::{ProgressBar, Units};

pub fn download_client() -> Client {
    Client::builder()
        .timeout(Duration::new(5, 0))
        .build()
        .unwrap()
}

pub fn download(client: &Client, remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let mut response = match client.get(remote_path).send() {
        Ok(response) => response,
        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
    };

    match response.status() {
        StatusCode::OK => {
            let mut count = 0;
            let length = response.content_length().unwrap_or(0);

            let mut file = File::create(&local_path)?;
            let mut pb = ProgressBar::new(length as u64);
            pb.set_max_refresh_rate(Some(Duration::new(1, 0)));
            pb.set_units(Units::Bytes);
            loop {
                let mut buf = [0; 8192];
                let res = response.read(&mut buf)?;
                if res == 0 {
                    break;
                }
                count += file.write(&buf[..res])?;
                pb.set(count as u64);
            }
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
