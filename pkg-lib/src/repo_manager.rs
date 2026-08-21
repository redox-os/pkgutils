use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use std::{fs, path::PathBuf};

use crate::backend::wrap_io_err;
use crate::callback::Callback;
use crate::net_backend::DownloadError;
use crate::net_backend::{DownloadBackend, DownloadBackendWriter};
use crate::package::RemoteName;
use crate::{backend::Error, package::PackageError, PackageName};
use crate::{DOWNLOAD_DIR, PACKAGES_REMOTE_DIR};
use serde_derive::{Deserialize, Serialize};
/// Remote package management
pub struct RepoManager {
    /// http sources
    pub remotes: Vec<RemoteName>,
    /// file sources
    pub locals: Vec<RemoteName>,
    /// detailed http + file sources
    pub remote_map: BTreeMap<RemoteName, RemotePath>,
    pub download_path: PathBuf,
    pub download_backend: Rc<Box<dyn DownloadBackend>>,

    pub callback: Rc<RefCell<dyn Callback>>,
}

impl Debug for RepoManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoManager")
            .field("remotes", &self.remotes)
            .field("locals", &self.locals)
            .field("remote_map", &self.remote_map)
            .field("download_path", &self.download_path)
            .finish()
    }
}

impl Clone for RepoManager {
    fn clone(&self) -> Self {
        Self {
            remotes: self.remotes.clone(),
            locals: self.locals.clone(),
            remote_map: self.remote_map.clone(),
            download_path: self.download_path.clone(),
            download_backend: self.download_backend.clone(),
            callback: self.callback.clone(),
        }
    }
}

/// same as pkgar_core::PublicKey
pub type RepoPublicKey = [u8; 32];

#[derive(Clone, Debug, Deserialize, Serialize)]

/// same as pkgar_keys::PublicKeyFile
pub struct RepoPublicKeyFile {
    #[serde(
        serialize_with = "hex::serialize",
        deserialize_with = "hex::deserialize"
    )]
    pub pkey: RepoPublicKey,
}

impl RepoPublicKeyFile {
    pub fn new(pubkey: RepoPublicKey) -> Self {
        Self { pkey: pubkey }
    }

    pub fn open(file: impl AsRef<Path>) -> Result<RepoPublicKeyFile, Error> {
        let file = file.as_ref();
        let content = fs::read_to_string(file).map_err(wrap_io_err!(file, "Reading"))?;
        toml::from_str(&content).map_err(Error::TomlRead)
    }

    pub fn save(&self, file: impl AsRef<Path>) -> Result<(), Error> {
        let file = file.as_ref();
        fs::write(file, toml::to_string(&self).unwrap()).map_err(wrap_io_err!(file, "Writing"))
    }
}

#[derive(Clone, Debug)]
pub struct RemotePath {
    /// URL/Path to packages
    pub path: String,
    /// URL to public key
    pub pubpath: String,
    /// Unique ID
    pub name: RemoteName,
    /// Embedded public key, lazily loaded
    pub pubkey: Option<RepoPublicKey>,
}

impl RemotePath {
    pub fn is_local(&self) -> bool {
        self.pubpath.is_empty()
    }
}

const PUB_TOML: &str = "id_ed25519.pub.toml";

impl RepoManager {
    pub fn new(
        callback: Rc<RefCell<dyn Callback>>,
        download_backend: Box<dyn DownloadBackend>,
    ) -> Self {
        Self {
            remotes: Vec::new(),
            locals: Vec::new(),
            download_path: DOWNLOAD_DIR.into(),
            download_backend: Rc::new(download_backend),
            callback,
            remote_map: BTreeMap::new(),
        }
    }

    /// override from default
    pub fn set_download_path(&mut self, path: PathBuf) {
        self.download_path = path;
    }

    /// override from existing callback
    pub fn set_callback(&mut self, callback: Rc<RefCell<dyn Callback>>) {
        self.callback = callback;
    }

    /// read [install_path]/etc/pkg.d with specified target. Will reset existing remotes / locals list.
    pub fn update_remotes(&mut self, target: &str, install_path: &Path) -> Result<(), Error> {
        self.remotes = Vec::new();
        self.locals = Vec::new();
        self.remote_map = BTreeMap::new();

        let repos_path = install_path.join(PACKAGES_REMOTE_DIR);
        let mut repo_files = Vec::new();
        for entry_res in
            fs::read_dir(&repos_path).map_err(wrap_io_err!(&repos_path, "Reading dir"))?
        {
            let entry = entry_res.map_err(wrap_io_err!(&repos_path, "Reading dir item"))?;
            let path = entry.path();
            if path.is_file() {
                repo_files.push(path);
            }
        }
        repo_files.sort();
        for repo_file in repo_files {
            let data =
                fs::read_to_string(&repo_file).map_err(wrap_io_err!(&repo_file, "Reading"))?;
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

    fn extract_host(path: &str) -> Option<&str> {
        path.split("://")
            .nth(1)?
            .split('/')
            .next()?
            .split(':')
            .next()
    }

    /// Add a remote target. The domain url will be used as a host (unique identifier).
    pub fn add_remote(&mut self, url: &str, target: &str) -> Result<(), Error> {
        let host = Self::extract_host(url)
            .ok_or_else(|| Error::RepoPathInvalid(url.into()))?
            .to_string();

        if self
            .remote_map
            .insert(
                host.clone(),
                RemotePath {
                    path: format!("{}/{}", url, target),
                    pubpath: format!("{}/{}", url, PUB_TOML),
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

    /// Add a local directory target. Specify a host as a unique identifier.
    pub fn add_local(
        &mut self,
        host: &str,
        path: &str,
        target: &str,
        pubkey_dir: &Path,
    ) -> Result<(), Error> {
        let pubkey_path = pubkey_dir.join(PUB_TOML);
        if !pubkey_path.is_file() {
            return Err(Error::RepoPathInvalid(
                pubkey_path.to_string_lossy().to_string(),
            ));
        }
        // load to check for failure early
        let pubkey = RepoPublicKeyFile::open(&pubkey_path).inspect_err(|e| {
            // probably corrupted
            let _ = fs::remove_file(&pubkey_path);
        })?;
        if self
            .remote_map
            .insert(
                host.into(),
                RemotePath {
                    path: if path.is_empty() {
                        path.into()
                    } else {
                        format!("{}/{}", path, target)
                    },
                    // signifies local repository
                    pubpath: "".into(),
                    name: host.into(),
                    pubkey: Some(pubkey.pkey),
                },
            )
            .is_none()
        {
            self.locals.push(host.into());
        };
        Ok(())
    }

    /// Download a toml file. Wrapper to local_search() + download().
    fn sync_toml(&self, package_name: &PackageName) -> Result<(String, RemoteName), Error> {
        let file = format!("{package_name}.toml");
        if let Some((r, path)) = self.local_search(&file)? {
            let toml = fs::read_to_string(&path).map_err(wrap_io_err!(&path, "Reading"))?;
            return Ok((toml, r));
        }
        let mut writer = DownloadBackendWriter::ToBuf(Vec::new());
        match self.download(&file, None, None, &mut writer) {
            Ok(r) => {
                let text = writer.to_inner_buf();
                let toml = String::from_utf8(text)
                    .map_err(|_| Error::ContentIsNotValidUnicode(file))?;
                Ok((toml, r))
            }
            Err(Error::ValidRepoNotFound) => {
                Err(PackageError::PackageNotFound(package_name.to_owned()).into())
            }
            Err(e) => Err(e),
        }
    }

    /// Download a pkgar file to specified path. Wrapper to local_search() + download().
    fn sync_pkgar(
        &self,
        package_name: &PackageName,
        len_hint: u64,
        dst_path: PathBuf,
    ) -> Result<(PathBuf, RemoteName), Error> {
        let file = format!("{package_name}.pkgar");
        if let Some((r, path)) = self.local_search(&file)? {
            return Ok((path, r));
        }
        let mut writer = DownloadBackendWriter::ToFile(
            File::create(&dst_path).map_err(wrap_io_err!(&dst_path, "Creating"))?,
        );
        match self.download(&file, Some(len_hint), None, &mut writer) {
            Ok(r) => Ok((dst_path, r)),
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
        self.sync_keys_internal(false, false)
    }

    /// Downloads all keys forcibly
    pub fn force_sync_keys(&mut self) -> Result<(), Error> {
        self.sync_keys_internal(true, false)
    }

    /// Downloads all keys forcibly for testing
    pub fn test_sync_keys(&mut self) -> Result<(), Error> {
        self.sync_keys_internal(true, true)
    }

    fn sync_keys_internal(&mut self, force: bool, cleanup: bool) -> Result<(), Error> {
        let download_dir = &self.download_path;
        if !download_dir.is_dir() {
            fs::create_dir_all(download_dir)
                .map_err(wrap_io_err!(&download_dir, "Creating dir"))?;
        }
        for (_, remote) in self.remote_map.iter_mut() {
            if remote.pubkey.is_some() {
                continue;
            }
            // download key if not exists
            if force || remote.pubkey.is_none() {
                let local_keypath = download_dir.join(format!("pub_key_{}.toml", remote.name));
                if force || !local_keypath.exists() {
                    self.download_backend.download_to_file(
                        &remote.pubpath,
                        None,
                        &local_keypath,
                        self.callback.clone(),
                    )?;
                }
                let pubkey = RepoPublicKeyFile::open(&local_keypath).inspect_err(|e| {
                    // probably corrupted
                    let _ = fs::remove_file(&local_keypath);
                })?;
                if cleanup {
                    let _ = fs::remove_file(&local_keypath);
                }
                remote.pubkey = Some(pubkey.pkey);
            }
        }

        Ok(())
    }

    /// Download to dest and report which remotes it's downloaded from.
    pub fn download(
        &self,
        file: &str,
        len: Option<u64>,
        remote: Option<&RemoteName>,
        dest: &mut DownloadBackendWriter,
    ) -> Result<RemoteName, Error> {
        if !self.download_path.exists() {
            fs::create_dir_all(self.download_path.clone())
                .map_err(wrap_io_err!(&self.download_path, "Creating dir"))?;
        }

        match remote {
            Some(rname) => {
                if let Some(remote) = self.remote_map.get(rname) {
                    if !remote.path.is_empty() {
                        return match self.download_inner(remote, file, len, dest) {
                            Ok(_) => Ok(rname.into()),
                            Err(e) => Err(Error::Download(e)),
                        };
                    }
                }
            }
            None => {
                for rname in self.remotes.iter() {
                    let Some(remote) = self.remote_map.get(rname) else {
                        continue;
                    };
                    if remote.path.is_empty() {
                        // installer repository
                        continue;
                    }
                    match self.download_inner(remote, file, len, dest) {
                        Ok(_) => return Ok(rname.into()),
                        Err(DownloadError::HttpStatus(_)) => continue,
                        Err(e) => {
                            return Err(Error::Download(e));
                        }
                    };
                }
            }
        }

        Err(Error::ValidRepoNotFound)
    }

    fn download_inner(
        &self,
        remote: &RemotePath,
        file: &str,
        len: Option<u64>,
        mut dest: &mut DownloadBackendWriter,
    ) -> Result<(), DownloadError> {
        let remote_path = format!("{}/{}", remote.path, file);
        self.download_backend
            .download(&remote_path, len, dest, self.callback.clone())
    }

    /// Locate and return path and report which locals it's downloaded from.
    pub fn local_search(&self, file: &str) -> Result<Option<(RemoteName, PathBuf)>, Error> {
        if !self.download_path.exists() {
            fs::create_dir_all(self.download_path.clone())
                .map_err(wrap_io_err!(self.download_path, "Creating directory"))?;
        }

        for rname in self.locals.iter() {
            let Some(remote) = self.remote_map.get(rname) else {
                continue;
            };
            if remote.path.is_empty() {
                // installer repository
                continue;
            }

            let remote_path = Path::new(&remote.path).join(file);
            match remote_path.metadata() {
                Ok(e) => {
                    if e.is_file() {
                        return Ok(Some((rname.into(), remote_path)));
                    } else {
                        continue;
                    }
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        continue;
                    } else {
                        return Err(Error::IO(err, remote_path, "Reading metadata"));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Download a pkgar file to the download path. Wrapper to sync_pkgar().
    pub fn get_package_pkgar(
        &self,
        package: &PackageName,
        len_hint: u64,
    ) -> Result<(PathBuf, &RemotePath), Error> {
        let local_path = self.get_local_path(&"".to_string(), package.as_str(), "pkgar");
        let (local_path, remote) = self.sync_pkgar(package, len_hint, local_path)?;
        if let Some(r) = self.remote_map.get(&remote) {
            if r.is_local() {
                return Ok((local_path, r));
            }
            let new_local_path = self.get_local_path(&r.name, package.as_str(), "pkgar");
            if new_local_path != local_path {
                fs::rename(&local_path, &new_local_path)
                    .map_err(wrap_io_err!(new_local_path, "Renaming"))?;
            }
            Ok((new_local_path, r))
        } else {
            // the pubkey cache is failing to download?
            Err(Error::RepoCacheNotFound(package.clone()))
        }
    }

    /// Fetch a toml file. Wrapper to sync_toml() with notifies fetch callback.
    pub fn get_package_toml(&self, package: &PackageName) -> Result<(String, RemoteName), Error> {
        self.callback.borrow_mut().fetch_package_name(package);
        self.sync_toml(package)
    }

    /// Get remote info, if available
    pub fn get_remote_info(&self, remote: &RemoteName) -> Option<&RemotePath> {
        self.remote_map.get(remote)
    }
}
