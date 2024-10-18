use std::cell::RefCell;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::net_backend::DownloadBackend;
use crate::{backend::Error, Callback, PackageName};

pub struct RepoManager {
    pub remotes: Vec<String>,
    pub download_path: PathBuf,
    pub download_backend: Box<dyn DownloadBackend>,

    pub callback: Rc<RefCell<dyn Callback>>,
}

impl RepoManager {
    pub fn sync_toml(&self, package_name: &PackageName) -> String {
        //TODO: just load directly into memory
        self.sync_and_read(&format!("{package_name}.toml")).unwrap()
    }

    pub fn sync_pkgar(&self, package_name: &PackageName) {
        self.sync(&format!("{package_name}.pkgar")).unwrap()
    }

    pub fn sync_website(&self) -> String {
        let local_path = &self.download_path.join("website");

        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        for remote in self.remotes.iter() {
            match self
                .download_backend
                .download(remote, local_path, self.callback.clone())
            {
                Ok(()) => {
                    break;
                }
                Err(err) => {
                    eprintln!(
                        "failed to download {:?} to {:?}: {}",
                        remote, local_path, err
                    );
                }
            }
        }
        fs::read_to_string(self.download_path.join("website")).unwrap()
    }

    pub fn sync(&self, file: &str) -> Result<(), Error> {
        let local_path = self.download_path.join(file);

        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)?;
        }

        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}", remote, file);
            let res =
                self.download_backend
                    .download(&remote_path, &local_path, self.callback.clone());
            if res.is_ok() {
                return Ok(());
            }
        }

        Err(Error::ValidRepoNotFound)
    }

    pub fn sync_and_read(&self, file: &str) -> Result<String, Error> {
        self.sync(file)?;

        Ok(fs::read_to_string(self.download_path.join(file))?)
    }
}
