use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::time::Duration;

use pbr::{ProgressBar, Units};

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let mut stderr = stderr();

    write!(stderr, "* Requesting {}\n", remote_path)?;

    let response = ureq::get(remote_path)
        .timeout_read(5000)
        .timeout_write(5000)
        .call();

    if response.ok() {
        let mut count = 0;
        let length = response.header("Content-Length").expect("Can't get content-length");
        let length: u64 = length.parse().expect("Unable to parse content-length");
        let mut file = File::create(&local_path)?;
        let mut pb = ProgressBar::new(length);
        pb.set_max_refresh_rate(Some(Duration::new(1, 0)));
        pb.set_units(Units::Bytes);

        let mut reader = response.into_reader();

        loop {
            let mut buf = [0; 8192];
            let res = reader.read(&mut buf).expect("Read failed");
            if res == 0 {
                break;
            }
            count += file.write(&buf[..res]).expect("Write failed");
            pb.set(count as u64);
        }

        let _ = write!(stderr, "\n");

        file.sync_all()?;

        Ok(())
    } else {
        let _ = write!(stderr, "* Failure {}\n", response.status());

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} not found", remote_path),
        ))
    }
}
