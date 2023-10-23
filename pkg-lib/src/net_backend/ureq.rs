use std::{path::Path, fs::File, io::Write};

use super::{DownloadBackend, Callback, DownloadError};
use std::io::Read;


#[derive(Clone, Copy)]
pub struct UreqBackend {}

impl DownloadBackend for UreqBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {

        let resp = ureq::get(remote_path)
            .call();//.unwrap();

        let len: u64 = resp.header("Content-Length").unwrap_or("0").parse().unwrap();

        let mut output = File::create(local_path)?;
        let mut offset = 0;
        
        callback.start(len, remote_path);

        let body = resp.into_reader().bytes();

        body.for_each(|x| {

            output.write_all(&[x.unwrap()]).unwrap();

            offset += 1;

            callback.update(offset);
        });

        callback.end();

        Ok(())
    }
}
