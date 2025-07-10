use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::net_backend::DownloadBackend;
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
    pub path: String,
    pub key: String,
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
        let local_keypath = Path::new(&pubkey);
        let remote_keypath = format!("{}/{}", path, PUB_TOML);

        if !self.prefer_cache || !local_keypath.exists() {
            self.download_backend.download(
                &remote_keypath,
                local_keypath,
                self.callback.clone(),
            )?;
        }

        self.remotes.push(RemotePath {
            path: format!("{}/{}", path, target),
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
                // delete cache
                let _ = fs::remove_file(self.download_path.join(&file_name));
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    pub fn sync(&self, file: &str) -> Result<&RemotePath, Error> {
        let local_path = self.download_path.join(file);

        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if self.prefer_cache && local_path.exists() {
            return Err(Error::RepoCacheExists(local_path));
        }

        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}", remote.path, file);
            let res =
                self.download_backend
                    .download(&remote_path, &local_path, self.callback.clone());
            if res.is_ok() {
                return Ok(remote);
            }
        }

        Err(Error::ValidRepoNotFound)
    }

    pub fn sync_and_read(&self, file: &str) -> Result<String, Error> {
        match self.sync(file) {
            Ok(_) => Ok(fs::read_to_string(self.download_path.join(file))?),
            Err(Error::RepoCacheExists(path)) => Ok(fs::read_to_string(path)?),
            Err(e) => Err(e),
        }
    }
}
