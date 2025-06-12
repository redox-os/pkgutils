use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::net_backend::DownloadBackend;
use crate::{backend::Error, Callback, PackageName};
use reqwest::Url;

pub struct RepoManager {
    pub remotes: Vec<RemotePath>,
    pub download_path: PathBuf,
    pub download_backend: Box<dyn DownloadBackend>,

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
        let remote_keypath = format!("{}/{}", path, PUB_TOML);
        self.download_backend
            .download(&remote_keypath, Path::new(&pubkey), self.callback.clone())?;

        self.remotes.push(RemotePath {
            path: format!("{}/{}", path, target),
            key: host,
            pubkey,
        });

        return Ok(());
    }

    pub fn sync_toml(&self, package_name: &PackageName) -> Result<String, Error> {
        //TODO: just load directly into memory
        match self.sync_and_read(&format!("{package_name}.toml")) {
            Ok(toml) => Ok(toml),
            Err(Error::ValidRepoNotFound) => Err(Error::PackageNotFound(package_name.to_owned())),
            Err(e) => Err(e),
        }
    }

    pub fn sync_pkgar(&self, package_name: &PackageName) -> Result<&RemotePath, Error> {
        match self.sync(&format!("{package_name}.pkgar")) {
            Ok(r) => Ok(r),
            Err(Error::ValidRepoNotFound) => Err(Error::PackageNotFound(package_name.to_owned())),
            Err(e) => Err(e),
        }
    }

    pub fn sync(&self, file: &str) -> Result<&RemotePath, Error> {
        let local_path = self.download_path.join(file);

        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)?;
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
        self.sync(file)?;

        Ok(fs::read_to_string(self.download_path.join(file))?)
    }
}
