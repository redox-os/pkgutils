use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::net_backend::{DownloadBackend, DownloadError};
use crate::{backend::Error, package::PackageError, Callback, PackageName};
use reqwest::Url;

pub struct RepoManager {
    pub remotes: Vec<RemotePath>,
    pub download_path: PathBuf,
    pub download_backend: Box<dyn DownloadBackend>,
    pub prefer_cache: bool,

    pub callback: Rc<RefCell<dyn Callback>>,
}

pub struct RemotePath {
    /// HTTP URI to packages
    pub path: String,
    /// HTTP URI to public key
    pub pubpath: String,
    /// Unique ID
    pub key: String,
    /// Local path to public key
    pub pubkey: String,
}

const PUB_DIR: &str = "/tmp/pkg";
const PUB_TOML: &str = "id_ed25519.pub.toml";

impl RepoManager {
    pub fn add_remote(&mut self, path: &str, target: &str) -> Result<(), Error> {
        let host = Url::parse(path)
            .or(Err(Error::RepoPathInvalid))?
            .host_str()
            .ok_or(Error::RepoPathInvalid)?
            .to_owned();
        fs::create_dir_all(PUB_DIR)?;
        let pubkey = format!("{}/pub_key_{}.toml", PUB_DIR, host);

        self.remotes.push(RemotePath {
            path: format!("{}/{}", path, target),
            pubpath: format!("{}/{}", path, PUB_TOML),
            key: host,
            pubkey,
        });

        Ok(())
    }

    pub fn sync_toml(&self, package_name: &PackageName) -> Result<String, Error> {
        //TODO: just load directly into memory
        match self.sync_and_read(&format!("{package_name}.toml")) {
            Ok(toml) => Ok(toml),
            Err(Error::ValidRepoNotFound) => {
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    pub fn sync_pkgar(&self, package_name: &PackageName) -> Result<&RemotePath, Error> {
        let file_name = format!("{package_name}.pkgar");
        match self.sync(&file_name) {
            Ok(r) => Ok(r),
            Err(Error::ValidRepoNotFound) => {
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_local_path(&self, remote: &RemotePath, file: &str) -> PathBuf {
        self.download_path.join(format!("{}_{file}", remote.key))
    }

    pub fn sync_keys(&self) -> Result<(), Error> {
        for remote in self.remotes.iter() {
            // download key if not exists
            let local_keypath = Path::new(&remote.pubkey);
            if !local_keypath.exists() {
                self.download_backend.download(
                    &remote.pubpath,
                    local_keypath,
                    self.callback.clone(),
                )?;
            }
        }

        Ok(())
    }

    pub fn sync(&self, file: &str) -> Result<&RemotePath, Error> {
        if !self.download_path.exists() {
            fs::create_dir_all(self.download_path.clone())?;
        }

        self.sync_keys()?;

        for remote in self.remotes.iter() {
            let local_path = self.get_local_path(remote, file);

            if self.prefer_cache && local_path.exists() {
                // confidently trust this cached package
                // pkgar backend will verify it
                return Ok(remote);
            }

            let remote_path = format!("{}/{}", remote.path, file);
            let res =
                self.download_backend
                    .download(&remote_path, &local_path, self.callback.clone());
            match res {
                Ok(_) => return Ok(remote),
                Err(DownloadError::HttpStatus(_)) => continue,
                Err(e) => {
                    // delete cache if any
                    let _ = fs::remove_file(&local_path);
                    return Err(Error::Download(e));
                }
            };
        }

        Err(Error::ValidRepoNotFound)
    }

    pub fn sync_and_read(&self, file: &str) -> Result<String, Error> {
        match self.sync(file) {
            Ok(r) => Ok(fs::read_to_string(self.get_local_path(r, file))?),
            Err(e) => Err(e),
        }
    }
}
