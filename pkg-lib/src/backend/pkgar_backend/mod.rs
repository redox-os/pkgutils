use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::PublicKeyFile;

use self::packages::Packages;
use super::{Backend, Error};
use crate::{
    package::Repository,
    repo_manager::{RemotePath, RepoManager},
    Package, PackageName, DOWNLOAD_PATH, PACKAGES_PATH,
};

mod packages;

pub struct PkgarBackend {
    install_path: PathBuf,
    packages: Packages,
    repo_manager: RepoManager,
    pkey_files: HashMap<String, PublicKeyFile>,
}

const PACKAGES_DIR: &str = "pkg/packages";

impl PkgarBackend {
    pub fn new<P: AsRef<Path>>(install_path: P, repo_manager: RepoManager) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let packages_path = install_path.join(PACKAGES_PATH);
        let packages_dir = install_path.join(PACKAGES_DIR);
        let file = fs::read_to_string(&packages_path);

        let packages;
        match file {
            Ok(toml) => {
                packages = Packages::from_toml(&toml)?;
            }
            Err(_) => {
                packages = Packages::default();
                fs::create_dir_all(Path::new(&packages_path).parent().unwrap())?;
            }
        }

        // TODO: Use File::lock. This only checks permission
        fs::write(packages_path, packages.to_toml())?;
        fs::create_dir_all(&packages_dir)?;

        let mut pkey_files = HashMap::new();
        for remote in repo_manager.remotes.iter() {
            pkey_files.insert(
                remote.key.clone(),
                PublicKeyFile::open(remote.pubkey.clone())?,
            );
        }

        Ok(PkgarBackend {
            install_path: install_path.to_path_buf(),
            packages,
            repo_manager,
            pkey_files,
        })
    }

    fn get_package_head(&mut self, package: &PackageName) -> Result<PackageFile, Error> {
        let path = self
            .install_path
            .join(PACKAGES_DIR)
            .join(format!("{package}.pkgar_head"));

        // TODO: A way to get chosen remote of a pkg so we can remove this trial loop
        for remote in self.repo_manager.remotes.iter() {
            let pubkey = self.pkey_files.get(&remote.key);
            if let Some(key) = pubkey {
                let pkg = PackageFile::new(&path, &key.pkey);
                if let Ok(p) = pkg {
                    return Ok(p);
                }
            }
        }
        Err(Error::RepoCacheNotFound(package.clone()))
    }

    fn get_package_pkgar(&self, package: &PackageName) -> Result<&RemotePath, Error> {
        let r = self.repo_manager.sync_pkgar(package);
        if let Err(Error::RepoCacheExists(path)) = &r {
            // cached locally, figure out what repo to return
            for remote in self.repo_manager.remotes.iter() {
                let pubkey = self.pkey_files.get(&remote.key);
                if let Some(key) = pubkey {
                    let pkg = PackageFile::new(path, &key.pkey);
                    if let Ok(_) = pkg {
                        return Ok(remote);
                    }
                }
            }

            return Err(Error::RepoCacheNotFound(package.clone()));
        }

        r
    }

    fn get_package(&self, package: &PackageName, repokey: &str) -> Result<PackageFile, Error> {
        Ok(PackageFile::new(
            format!("{}/{package}.pkgar", DOWNLOAD_PATH),
            &self
                .pkey_files
                .get(repokey)
                .ok_or(Error::ValidRepoNotFound)?
                .pkey,
        )?)
    }

    fn get_package_toml(&self, package: &PackageName) -> Result<String, Error> {
        self.repo_manager.sync_toml(package)
    }

    fn remove_package_head(&mut self, package: &PackageName) -> Result<(), Error> {
        let path = self
            .install_path
            .join(PACKAGES_DIR)
            .join(format!("{package}.pkgar_head"));

        fs::remove_file(path)?;
        Ok(())
    }

    fn create_head(&mut self, package: &PackageName, pubkey_path: &str) -> Result<(), Error> {
        // creates a head file
        pkgar::split(
            pubkey_path,
            format!("{}/{package}.pkgar", DOWNLOAD_PATH),
            self.install_path
                .join(PACKAGES_DIR)
                .join(format!("{package}.pkgar_head")),
            Option::<&str>::None,
        )?;

        Ok(())
    }
}

impl Backend for PkgarBackend {
    fn install(&mut self, package: PackageName) -> Result<(), Error> {
        let repo = self.get_package_pkgar(&package)?;
        let mut pkg = self.get_package(&package, &repo.key)?;
        let pubkey_path = repo.pubkey.clone();
        let mut install = Transaction::install(&mut pkg, &self.install_path)?;
        install.commit()?;

        self.create_head(&package, &pubkey_path)?;

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

        let repo = self.get_package_pkgar(&package)?;
        let pubkey_path = repo.pubkey.clone();

        let mut pkg2 = self.get_package(&package, &repo.key)?;

        let mut update = Transaction::replace(&mut pkg, &mut pkg2, &self.install_path)?;
        update.commit()?;

        self.create_head(&package, &pubkey_path)?;

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
        let entries = fs::read_dir(self.install_path.join(PACKAGES_DIR))?;

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
        let packages_path = self.install_path.join(PACKAGES_PATH);
        // we already check permissions before so the error can be ignored
        let _ = fs::write(packages_path, self.packages.to_toml());
    }
}
