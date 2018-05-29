use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::error::Error;

use reqwest::{self, header::ContentLength, StatusCode};

use pbr::{ProgressBar, Units};

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let mut response = match reqwest::get(remote_path) {
        Ok(response) => response,
        Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err.description()))
    };

    match response.status() {
        StatusCode::Ok => {
            let mut count = 0;
            let length = response.headers().get::<ContentLength>().map_or(0, |h| h.0 as usize);

            let mut file = File::create(&local_path)?;
            let mut pb = ProgressBar::new(length as u64);

            pb.set_units(Units::Bytes);
            loop {
                let mut buf = [0; 8192];
                let res = response.read(&mut buf)?;
                if res == 0 {
                    break;
                }
                file.write_all(&buf[..res])?;
                count += res;
                pb.set(count as u64);
            }
            let _ = write!(stderr, "\n");

            file.sync_all()?;

            Ok(())
        },
        _ => {
            let _ = write!(stderr, "* Failure {}\n", response.status());

            Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", remote_path)))
        }
    }
}
