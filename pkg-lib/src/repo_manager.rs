use std::{fs, path::PathBuf};

use crate::{backend::Error, net_backend::Callback};
use crate::net_backend::DownloadBackend;

pub struct RepoManager {
    pub remotes: Vec<String>,
    pub download_path: PathBuf,
    pub download_backend: Box<dyn DownloadBackend>,
}

impl RepoManager {
    pub fn sync(&self, file: &str, callback: &mut dyn Callback) -> Result<(), Error> {
        let local_path = if file.is_empty() {
            self.download_path.join("website")
        } else {
            self.download_path.join(file)
        };

        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)?;
        }

        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}", remote, file);
            let res = self
                .download_backend
                .download(&remote_path, &local_path, callback);
            return Ok(res.unwrap());
        }

        Err(Error::NoReposWereAdded)
    }

    pub fn sync_and_read(&self, file: &str, callback: &mut dyn Callback) -> Result<String, Error> {
        self.sync(file, callback)?;

        Ok(fs::read_to_string(self.download_path.join(file))?)
    }
}
