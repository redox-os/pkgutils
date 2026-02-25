use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::PublicKeyFile;

use super::{Backend, Error};
use crate::{
    package::Repository,
    package_state::PackageState,
    repo_manager::{RemotePath, RepoManager},
    Package, PackageName,
};

/// Package backend using pkgar
pub struct PkgarBackend {
    /// Root path, usually "/"
    install_path: PathBuf,
    /// Things in "/var/pkg"
    packages: PackageState,
    /// Things in "/etc/pkg.d"
    repo_manager: RepoManager,
}

// FIXME: can't use repo_manager.get_local_path because of borrowing rules
fn get_package_path(repokey: &str, package: &PackageName) -> PathBuf {
    let local_file = format!("{}_{}.pkgar", repokey, package.as_str());
    PathBuf::from(crate::DOWNLOAD_DIR).join(local_file)
}
fn get_package(
    repokey: &str,
    package: &PackageName,
    pubkey: &PublicKeyFile,
) -> Result<PackageFile, Error> {
    let local_path = get_package_path(repokey, package);
    Ok(PackageFile::new(local_path, &pubkey.pkey)?)
}

// FIXME: can't use self.pkey_files because of borrowing rules
fn get_pkey_file<'a>(
    key: &'a str,
    pkey_files: &'a mut BTreeMap<String, PublicKeyFile>,
    repo_manager: &'a RepoManager,
) -> Result<Option<&'a PublicKeyFile>, Error> {
    if pkey_files.get(key).is_none() {
        for remote in repo_manager.remotes.iter() {
            if remote.key == key {
                pkey_files.insert(
                    remote.key.clone(),
                    PublicKeyFile::open(remote.pubkey.clone())?,
                );
            }
        }
    }

    if let Some(value) = pkey_files.get(key) {
        return Ok(Some(value));
    }

    Ok(None)
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
        fs::write(packages_path, packages.to_toml())?;
        fs::create_dir_all(&packages_dir)?;

        Ok(PkgarBackend {
            install_path: install_path.to_path_buf(),
            packages,
            repo_manager,
        })
    }

    // reads /var/lib/packages/[package].pkgar_head
    fn get_package_head(&mut self, package: &PackageName) -> Result<PackageFile, Error> {
        let path = self
            .install_path
            .join(crate::PACKAGES_HEAD_DIR)
            .join(format!("{package}.pkgar_head"));

        self.repo_manager.sync_keys()?;

        // TODO: A way to get chosen remote of a pkg so we can remove this trial loop
        for remote in self.repo_manager.remotes.iter() {
            let pubkey =
                get_pkey_file(&remote.key, &mut self.packages.pubkeys, &self.repo_manager)?;
            if let Some(key) = pubkey {
                let pkg = PackageFile::new(&path, &key.pkey);
                if let Ok(p) = pkg {
                    return Ok(p);
                }
            }
        }
        Err(Error::RepoCacheNotFound(package.clone()))
    }

    // reads /tmp/pkg_download/[package].pkgar
    fn get_package_pkgar(
        &mut self,
        package: &PackageName,
    ) -> Result<(&RemotePath, &PublicKeyFile), Error> {
        let r = self.repo_manager.sync_pkgar(package)?;
        let pubkey = get_pkey_file(&r.key, &mut self.packages.pubkeys, &self.repo_manager)?;
        if let Some(pkey) = pubkey {
            Ok((r, pkey))
        } else {
            // the pubkey cache is failing to download?
            Err(Error::RepoCacheNotFound(package.clone()))
        }
    }

    // reads /tmp/pkg_download/[package].toml
    fn get_package_toml(&self, package: &PackageName) -> Result<String, Error> {
        self.repo_manager.sync_toml(package)
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
        &mut self,
        package: &PackageName,
        repokey: &str,
        pubkey_path: &str,
    ) -> Result<(), Error> {
        // creates a head file
        pkgar::split(
            pubkey_path,
            get_package_path(repokey, package),
            self.install_path
                .join(crate::PACKAGES_HEAD_DIR)
                .join(format!("{package}.pkgar_head")),
            Option::<&str>::None,
        )?;

        Ok(())
    }
}

impl Backend for PkgarBackend {
    fn install(&mut self, package: PackageName) -> Result<(), Error> {
        let (repo, pubkey) = self.get_package_pkgar(&package)?;
        let (repokey, pubkey_path) = (repo.key.clone(), repo.pubkey.clone());
        let mut pkg = get_package(&repo.key, &package, pubkey)?;
        let mut install = Transaction::install(&mut pkg, &self.install_path)?;
        install.commit()?;

        self.create_head(&package, &repokey, &pubkey_path)?;

        Ok(())
    }

    fn uninstall(&mut self, package: PackageName) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Err(Error::ProtectedPackage(package));
        }

        let mut pkg = self.get_package_head(&package)?;
        let mut remove = Transaction::remove(&mut pkg, &self.install_path)?;
        remove.commit()?;

        self.remove_package_head(&package)?;

        Ok(())
    }

    fn upgrade(&mut self, package: PackageName) -> Result<(), Error> {
        let mut pkg = self.get_package_head(&package)?;
        let (repo, pubkey) = self.get_package_pkgar(&package)?;
        let (repokey, pubkey_path) = (repo.key.clone(), repo.pubkey.clone());
        let mut pkg2 = get_package(&repo.key, &package, pubkey)?;
        let mut update = Transaction::replace(&mut pkg, &mut pkg2, &self.install_path)?;
        update.commit()?;

        self.create_head(&package, &repokey, &pubkey_path)?;

        Ok(())
    }

    fn get_package_detail(&self, package: &PackageName) -> Result<Package, Error> {
        let toml = self.get_package_toml(package)?;

        Ok(Package::from_toml(&toml)?)
    }

    fn get_repository_detail(&self) -> Result<Repository, Error> {
        let repo_str = PackageName::new("repo".to_string())?;
        let toml = self.get_package_toml(&repo_str)?;

        Ok(Repository::from_toml(&toml)?)
    }

    fn get_installed_packages(&self) -> Result<Vec<PackageName>, Error> {
        let entries = fs::read_dir(self.install_path.join(crate::PACKAGES_HEAD_DIR))?;

        let mut packages = vec![];

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_str().ok_or(Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                "file name isn't UTF-8",
            )))?;

            if file_name_str.ends_with(".pkgar_head") {
                let package = file_name_str.replace(".pkgar_head", "");
                packages.push(PackageName::new(package)?);
            }
        }

        Ok(packages)
    }
}

impl Drop for PkgarBackend {
    fn drop(&mut self) {
        let packages_path = self.install_path.join(crate::PACKAGES_TOML_PATH);
        // we already check permissions before so the error can be ignored
        let _ = fs::write(packages_path, self.packages.to_toml());
    }
}
