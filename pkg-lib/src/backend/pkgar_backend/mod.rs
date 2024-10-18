use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::PublicKeyFile;

use self::packages::Packages;
use super::{Backend, Callback, Error};
use crate::{repo_manager::RepoManager, PackageName, DOWNLOAD_PATH, PACKAGES_PATH};

mod packages;

pub struct PkgarBackend {
    install_path: PathBuf,
    packages: Packages,
    repo_manager: RepoManager,
    pkey_file: PublicKeyFile,
}

const PACKAGES_DIR: &str = "pkg/packages";

impl PkgarBackend {
    pub fn new<P: AsRef<Path>>(
        install_path: P,
        repo_manager: RepoManager,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let packages_path = install_path.join(PACKAGES_PATH);
        let file = fs::read_to_string(&packages_path);

        let packages;
        match file {
            Ok(toml) => {
                packages = Packages::from_toml(&toml)?;
            }

            Err(_) => {
                packages = Packages::default();
                fs::create_dir_all(Path::new(&packages_path).parent().unwrap())?;
                fs::write(packages_path, packages.to_toml())?;
            }
        }

        let packages_dir = install_path.join(PACKAGES_DIR);
        fs::create_dir_all(&packages_dir)?;

        fs::create_dir_all("/tmp/pkg/")?;
        repo_manager.download_backend.download(
            "https://static.redox-os.org/pkg/id_ed25519.pub.toml",
            Path::new("/tmp/pkg/pub_key.toml"),
            callback,
        )?;

        Ok(PkgarBackend {
            install_path: install_path.to_path_buf(),
            packages,
            repo_manager,
            pkey_file: PublicKeyFile::open("/tmp/pkg/pub_key.toml")?,
        })
    }

    fn get_package_head(&mut self, package: &PackageName) -> Result<PackageFile, Error> {
        let path = self
            .install_path
            .join(PACKAGES_DIR)
            .join(format!("{package}.pkgar_head"));

        Ok(PackageFile::new(path, &self.pkey_file.pkey)?)
    }

    fn get_package(&mut self, package: &PackageName) -> Result<PackageFile, Error> {
        Ok(PackageFile::new(
            format!("{}/{package}.pkgar", DOWNLOAD_PATH),
            &self.pkey_file.pkey,
        )?)
    }

    fn remove_package_head(&mut self, package: &PackageName) -> Result<(), Error> {
        let path = self
            .install_path
            .join(PACKAGES_DIR)
            .join(format!("{package}.pkgar_head"));

        fs::remove_file(path)?;
        Ok(())
    }

    fn create_head(&mut self, package: &PackageName) -> Result<(), Error> {
        // creates a head file
        pkgar::split(
            "/tmp/pkg/pub_key.toml",
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
        self.repo_manager.sync_pkgar(&package);

        let mut pkg = self.get_package(&package)?;

        let mut install = Transaction::install(&mut pkg, &self.install_path)?;
        install.commit()?;

        self.create_head(&package)?;

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

        self.repo_manager.sync_pkgar(&package);
        let mut pkg2 = self.get_package(&package)?;

        let mut update = Transaction::replace(&mut pkg, &mut pkg2, &self.install_path)?;
        update.commit()?;

        self.create_head(&package)?;

        Ok(())
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
        fs::write(packages_path, self.packages.to_toml()).unwrap();
    }
}
