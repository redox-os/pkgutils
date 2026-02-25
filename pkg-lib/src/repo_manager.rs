use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::callback::Callback;
use crate::net_backend::{DownloadBackend, DownloadBackendWriter, DownloadError};
use crate::{backend::Error, package::PackageError, PackageName};
use crate::{RemoteName, DOWNLOAD_DIR, PACKAGES_REMOTE_DIR};
use pkgar_core::PublicKey;
use pkgar_keys::PublicKeyFile;
use reqwest::Url;

pub struct RepoManager {
    pub remotes: Vec<RemoteName>,
    pub remote_map: BTreeMap<RemoteName, RemotePath>,
    pub download_path: PathBuf,
    pub download_backend: Box<dyn DownloadBackend>,

    pub callback: Rc<RefCell<dyn Callback>>,
}

#[derive(Clone)]
pub struct RemotePath {
    /// URL/Path to packages
    pub path: String,
    /// URL/Path to public key
    pub pubpath: String,
    /// Unique ID
    pub name: RemoteName,
    /// Embedded public key, lazily loaded
    pub pubkey: Option<PublicKey>,
}

const PUB_TOML: &str = "id_ed25519.pub.toml";

impl RepoManager {
    pub fn update_remotes(&mut self, target: &str, install_path: &Path) -> Result<(), Error> {
        self.remotes = Vec::new();
        self.remote_map = BTreeMap::new();

        let repos_path = install_path.join(PACKAGES_REMOTE_DIR);
        let mut repo_files = Vec::new();
        for entry_res in fs::read_dir(&repos_path)? {
            let entry = entry_res?;
            let path = entry.path();
            if path.is_file() {
                repo_files.push(path);
            }
        }
        repo_files.sort();
        for repo_file in repo_files {
            let data = fs::read_to_string(repo_file)?;
            for line in data.lines() {
                if !line.starts_with('#') {
                    self.add_remote(line.trim(), target)?;
                }
            }
        }
        // optional local path
        let local_pub_path = install_path.join("pkg");
        let _ = self.add_local("installer_key", "", target, &local_pub_path);
        Ok(())
    }

    pub fn add_remote(&mut self, path: &str, target: &str) -> Result<(), Error> {
        let host = Url::parse(path)
            .or(Err(Error::RepoPathInvalid))?
            .host_str()
            .ok_or(Error::RepoPathInvalid)?
            .to_owned();

        if self
            .remote_map
            .insert(
                host.clone(),
                RemotePath {
                    path: format!("{}/{}", path, target),
                    pubpath: format!("{}/{}", path, PUB_TOML),
                    name: host.clone(),
                    pubkey: None,
                },
            )
            .is_none()
        {
            self.remotes.push(host);
        };

        Ok(())
    }

    pub fn add_local(
        &mut self,
        host: &str,
        path: &str,
        target: &str,
        pubkey_dir: &Path,
    ) -> Result<(), Error> {
        let pubkey_path = pubkey_dir.join(PUB_TOML);
        if !pubkey_path.is_file() {
            return Err(Error::RepoPathInvalid);
        }
        // add_local can be mixed with remote net backend, so don't lazily load this
        let pubkey = pkgar_keys::PublicKeyFile::open(pubkey_path).map_err(Error::from)?;
        if self
            .remote_map
            .insert(
                host.into(),
                RemotePath {
                    path: format!("{}/{}", path, target),
                    pubpath: "".into(),
                    name: host.into(),
                    pubkey: Some(pubkey.pkey),
                },
            )
            .is_none()
        {
            self.remotes.push(host.into());
        };
        Ok(())
    }

    fn sync_toml(&self, package_name: &PackageName) -> Result<(String, RemoteName), Error> {
        match self.sync_and_read(&format!("{package_name}.toml")) {
            Ok(toml) => Ok(toml),
            Err(Error::ValidRepoNotFound) => {
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    fn sync_pkgar(&self, package_name: &PackageName, dst_path: &Path) -> Result<RemoteName, Error> {
        let mut file = DownloadBackendWriter::ToFile(File::create(&dst_path)?);
        match self.download(&format!("{package_name}.pkgar"), &mut file) {
            Ok(r) => Ok(r),
            Err(Error::ValidRepoNotFound) => {
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_local_path(&self, remote: &RemoteName, file: &str, ext: &str) -> PathBuf {
        self.download_path.join(format!("{}_{file}.{ext}", remote))
    }

    /// Downloads all keys
    pub fn sync_keys(&mut self) -> Result<(), Error> {
        let download_dir = Path::new(DOWNLOAD_DIR);
        if !download_dir.is_dir() {
            fs::create_dir_all(download_dir)?;
        }
        for (_, remote) in self.remote_map.iter_mut() {
            if remote.pubkey.is_some() {
                continue;
            }
            // download key if not exists
            if remote.pubkey.is_none() {
                let local_keypath = download_dir.join(format!("pub_key_{}.toml", remote.name));
                if !local_keypath.exists() {
                    self.download_backend.download_to_file(
                        &remote.pubpath,
                        &local_keypath,
                        self.callback.clone(),
                    )?;
                }
                let pubkey = PublicKeyFile::open(local_keypath)?;
                remote.pubkey = Some(pubkey.pkey);
            }
        }

        Ok(())
    }

    /// Download to dest and report which remote it's downloaded from.
    pub fn download(
        &self,
        file: &str,
        mut dest: &mut DownloadBackendWriter,
    ) -> Result<RemoteName, Error> {
        if !self.download_path.exists() {
            fs::create_dir_all(self.download_path.clone())?;
        }

        for rname in self.remotes.iter() {
            let Some(remote) = self.remote_map.get(rname) else {
                continue;
            };
            if remote.path == "" {
                // local repository
                continue;
            }

            let remote_path = format!("{}/{}", remote.path, file);
            let res =
                self.download_backend
                    .download(&remote_path, &mut dest, self.callback.clone());
            match res {
                Ok(_) => return Ok(rname.into()),
                Err(DownloadError::HttpStatus(_)) => continue,
                Err(e) => {
                    return Err(Error::Download(e));
                }
            };
        }

        Err(Error::ValidRepoNotFound)
    }

    pub fn sync_and_read(&self, file: &str) -> Result<(String, RemoteName), Error> {
        let mut writer = DownloadBackendWriter::ToBuf(Vec::new());
        match self.download(file, &mut writer) {
            Ok(r) => {
                let toml = String::from_utf8(writer.to_inner_buf())
                    .map_err(|_| Error::ContentIsNotValidUnicode(file.into()))?;
                Ok((toml, r))
            }
            Err(e) => Err(e),
        }
    }

    // downloads /tmp/pkg_download/[package].pkgar
    pub fn get_package_pkgar(
        &self,
        package: &PackageName,
    ) -> Result<(PathBuf, &RemotePath), Error> {
        let local_path = self.get_local_path(&"".to_string(), package.as_str(), "pkgar");
        let remote = self.sync_pkgar(&package, &local_path)?;
        if let Some(r) = self.remote_map.get(&remote) {
            Ok((local_path, r))
        } else {
            // the pubkey cache is failing to download?
            Err(Error::RepoCacheNotFound(package.clone()))
        }
    }

    // reads /tmp/pkg_download/[package].toml
    pub fn get_package_toml(&self, package: &PackageName) -> Result<(String, RemoteName), Error> {
        self.sync_toml(package)
    }
}
