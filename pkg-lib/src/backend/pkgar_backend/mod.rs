use std::{
    cell::RefCell,
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use pkgar::{PackageFile, Transaction};
use pkgar_core::PublicKey;
use pkgar_keys::PublicKeyFile;

use super::{Backend, Error};
use crate::{
    callback::Callback,
    package::{RemotePackage, Repository},
    package_state::PackageState,
    repo_manager::RepoManager,
    Package, PackageName,
};

/// Package backend using pkgar
pub struct PkgarBackend {
    /// Root path, usually "/"
    install_path: PathBuf,
    /// Things in "/etc/pkg/package.toml"
    packages: PackageState,
    /// Things in "/etc/pkg.d" and inet
    repo_manager: RepoManager,
    /// temporary commit
    commits: VecDeque<Transaction>,
    keys_synced: bool,
    callback: Rc<RefCell<dyn Callback>>,
}

impl PkgarBackend {
    pub fn new<P: AsRef<Path>>(install_path: P, repo_manager: RepoManager) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let packages_path = install_path.join(crate::PACKAGES_TOML_PATH);
        let packages_dir = install_path.join(crate::PACKAGES_HEAD_DIR);
        let file = fs::read_to_string(&packages_path);

        let packages;
        match file {
            Ok(toml) => {
                packages = PackageState::from_toml(&toml)?;
            }
            Err(_) => {
                packages = PackageState::default();
                fs::create_dir_all(Path::new(&packages_path).parent().unwrap())?;
            }
        }

        // TODO: Use File::lock. This only checks permission
        fs::write(&packages_path, packages.to_toml())?;

        fs::create_dir_all(&packages_dir)?;

        let callback = repo_manager.callback.clone();

        Ok(PkgarBackend {
            install_path: install_path.to_path_buf(),
            packages,
            repo_manager,
            // packages_lock,
            commits: VecDeque::new(),
            keys_synced: false,
            callback,
        })
    }

    // reads /var/lib/packages/[package].pkgar_head
    fn get_package_head(&self, package: &PackageName) -> Result<PackageFile, Error> {
        let path = self
            .install_path
            .join(crate::PACKAGES_HEAD_DIR)
            .join(format!("{package}.pkgar_head"));

        let Some(pkg) = self.packages.installed.get(package) else {
            return Err(Error::PackageNotInstalled(package.clone()));
        };
        let Some(remote) = self.packages.pubkeys.get(&pkg.remote) else {
            return Err(Error::RepoCacheNotFound(package.clone()));
        };

        let pkg = PackageFile::new(&path, &remote.pkey).map_err(Error::from)?;

        Ok(pkg)
    }

    fn remove_package_head(&mut self, package: &PackageName) -> Result<(), Error> {
        let path = self
            .install_path
            .join(crate::PACKAGES_HEAD_DIR)
            .join(format!("{package}.pkgar_head"));

        fs::remove_file(path)?;
        Ok(())
    }

    fn create_head(
        &self,
        archive_path: &Path,
        package: &PackageName,
        pubkey: &PublicKey,
    ) -> Result<(), Error> {
        // creates a head file
        let head_path = self
            .install_path
            .join(crate::PACKAGES_HEAD_DIR)
            .join(format!("{package}.pkgar_head"));

        let mut package = PackageFile::new(archive_path, &pubkey)?;
        package.split(&head_path, None)?;

        Ok(())
    }

    fn sync_keys(&mut self) -> Result<(), Error> {
        if self.keys_synced {
            return Ok(());
        }

        for (name, map) in &mut self.repo_manager.remote_map {
            if map.pubkey.is_none() {
                if let Some(pubk) = self.packages.pubkeys.get(name) {
                    map.pubkey = Some(pubk.pkey)
                }
            }
        }

        self.repo_manager.sync_keys()?;

        self.keys_synced = true;
        Ok(())
    }
}

impl Backend for PkgarBackend {
    fn install(&mut self, package: RemotePackage) -> Result<(), Error> {
        self.sync_keys()?;
        // TODO: Actually use that specific remote
        let (local_path, repo) = self
            .repo_manager
            .get_package_pkgar(&package.package.name, package.package.network_size)?;
        let mut pkg = PackageFile::new(&local_path, &repo.pubkey.unwrap())?;
        let install = Transaction::install(&mut pkg, &self.install_path)?;
        self.commits.push_back(install);
        self.create_head(&local_path, &package.package.name, &repo.pubkey.unwrap())?;
        Ok(())
    }

    fn uninstall(&mut self, package: PackageName) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Err(Error::ProtectedPackage(package));
        }
        self.sync_keys()?;

        let mut pkg = self.get_package_head(&package)?;
        let remove = Transaction::remove(&mut pkg, &self.install_path)?;
        self.commits.push_back(remove);

        self.remove_package_head(&package)?;

        Ok(())
    }

    fn upgrade(&mut self, package: PackageName) -> Result<(), Error> {
        self.sync_keys()?;

        let mut pkg = self.get_package_head(&package)?;
        let (local_path, repo) = self.repo_manager.get_package_pkgar(&package, 0)?;
        let mut pkg2 = PackageFile::new(&local_path, &repo.pubkey.unwrap())?;
        let update = Transaction::replace(&mut pkg, &mut pkg2, &self.install_path)?;
        self.commits.push_back(update);

        self.create_head(&local_path, &package, &repo.pubkey.unwrap())?;

        Ok(())
    }

    fn get_package_detail(&self, package: &PackageName) -> Result<RemotePackage, Error> {
        let (toml, remote) = self.repo_manager.get_package_toml(package)?;

        Ok(RemotePackage {
            package: Package::from_toml(&toml)?,
            remote,
        })
    }

    /// TODO: Multiple repository support
    fn get_repository_detail(&self) -> Result<Repository, Error> {
        let repo_str = PackageName::new("repo".to_string())?;
        let (toml, _) = self.repo_manager.get_package_toml(&repo_str)?;

        Ok(Repository::from_toml(&toml)?)
    }

    fn get_package_state(&self) -> PackageState {
        self.packages.clone()
    }

    fn commit_state(&mut self, new_state: PackageState) -> Result<usize, Error> {
        let mut total = 0;
        self.callback.borrow_mut().commit_start(self.commits.len());
        while let Some(mut commit) = self.commits.pop_front() {
            self.callback.borrow_mut().commit_increment(&commit);
            total += match commit.commit() {
                Ok(r) => r,
                Err(e) => {
                    self.commits.push_back(commit);
                    let err = Error::from(e);
                    return Err(err);
                }
            }
        }
        self.callback.borrow_mut().commit_end();

        self.packages = new_state;
        let packages_path = self.install_path.join(crate::PACKAGES_TOML_PATH);
        for (k, v) in &self.repo_manager.remote_map {
            let pk = PublicKeyFile::new(v.pubkey.unwrap());
            self.packages.pubkeys.insert(k.to_string(), pk);
        }
        fs::write(&packages_path, self.packages.to_toml())?;
        Ok(total)
    }

    fn abort_state(&mut self) -> Result<usize, Error> {
        let mut total = 0;
        self.callback.borrow_mut().abort_start(self.commits.len());
        while let Some(mut commit) = self.commits.pop_front() {
            self.callback.borrow_mut().abort_increment(&commit);
            total += match commit.abort() {
                Ok(_) => 1,
                Err(e) => {
                    let err = Error::from(e);
                    return Err(err);
                }
            };
        }
        self.callback.borrow_mut().abort_end();
        Ok(total)
    }
}
