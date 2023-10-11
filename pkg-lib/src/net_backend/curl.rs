use std::{path::Path, fs::File, io::Write};

use super::{DownloadBackend, Callback, DownloadError};


#[derive(Clone, Copy)]
pub struct CurlBackend {}

impl DownloadBackend for CurlBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {

        let mut curl = curl::easy::Easy::new();
        curl.url(remote_path).unwrap();

        let mut output = File::create(local_path)?;

        curl.write_function(move |data| {

            output.write_all(&data).unwrap();

            Ok(data.len())
        }).unwrap();

        callback.start(0, remote_path);

        curl.perform().unwrap();

        callback.end();


        Ok(())
    }
}
