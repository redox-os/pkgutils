use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{
    backend::{pkgar_backend::PkgarBackend, Backend, Error},
    callback::{Callback, SilentCallback},
    net_backend::DefaultNetBackend,
    net_backend::{DownloadBackend, DownloadError},
    RepoManager,
};

/// A struct to customize Library
pub struct LibraryBuilder {
    install_path: PathBuf,
    pub(crate) callback: Rc<RefCell<dyn Callback>>,
    download_backend: Result<Box<dyn DownloadBackend>, DownloadError>,
    nocheck: bool,
}

impl LibraryBuilder {
    pub fn new<P: AsRef<Path>>(install_path: P) -> Self {
        let callback = SilentCallback::new();
        let download_backend: Result<Box<dyn DownloadBackend>, DownloadError> =
            match DefaultNetBackend::new() {
                Ok(b) => Ok(Box::new(b)),
                Err(e) => Err(e),
            };
        Self {
            install_path: install_path.as_ref().to_path_buf(),
            callback: Rc::new(RefCell::new(callback)),
            download_backend,
            nocheck: false,
        }
    }
    pub fn with_install_path<P: Into<PathBuf>>(mut self, install_path: P) -> Self {
        self.install_path = install_path.into();
        self
    }
    pub fn with_callback(mut self, callback: Rc<RefCell<dyn Callback>>) -> Self {
        self.callback = callback;
        self
    }
    pub fn with_net_backend(mut self, callback: Box<dyn DownloadBackend>) -> Self {
        self.download_backend = Ok(callback);
        self
    }
    pub fn with_nocheck(mut self, nocheck: bool) -> Self {
        self.nocheck = nocheck;
        self
    }
    pub fn clone_with_net_backend(&self, backend: Box<dyn DownloadBackend>) -> Self {
        Self {
            install_path: self.install_path.clone(),
            callback: self.callback.clone(),
            download_backend: Ok(backend),
            nocheck: self.nocheck,
        }
    }
    pub fn build(
        self,
        remotes_fn: impl Fn(&mut RepoManager) -> Result<(), Error>,
    ) -> Result<Box<dyn Backend>, Error> {
        let mut repo_manager = RepoManager::new(self.callback, self.download_backend?);
        remotes_fn(&mut repo_manager)?;
        let mut backend = PkgarBackend::new(self.install_path, repo_manager)?;
        if self.nocheck {
            backend.set_nocheck(self.nocheck);
        }
        Ok(Box::new(backend))
    }
    pub fn install_path(&self) -> PathBuf {
        self.install_path.clone()
    }
}

impl TryFrom<LibraryBuilder> for RepoManager {
    type Error = Error;

    fn try_from(value: LibraryBuilder) -> Result<Self, Self::Error> {
        Ok(RepoManager::new(value.callback, value.download_backend?))
    }
}
