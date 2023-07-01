use std::{fs, path::Path};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::PublicKeyFile;

use crate::{repo_manager::RepoManager, DOWNLOAD_PATH, INSTALL_PATH, PACKAGES_PATH};

use self::packages::Packages;

use super::{Backend, Callback, Error};

mod packages;

struct NoCallback {}
impl Callback for NoCallback {
    fn start(&mut self, _: u64, _: &str) {}
    fn update(&mut self, _: u64) {}
    fn end(&mut self) {}
}

pub struct PkgarBackend {
    packages: Packages,
    repo_manager: RepoManager,
    pkey_file: Option<PublicKeyFile>,
}

const PACKAGES_DIR: &str = "pkg";

impl PkgarBackend {
    pub fn new(repo_manager: RepoManager) -> Result<Self, Error> {
        let packages;

        let packages_path = format!("{}/{}", INSTALL_PATH, PACKAGES_PATH);
        let file = fs::read_to_string(&packages_path);

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

        fs::create_dir_all(format!("{}/{}", INSTALL_PATH, PACKAGES_DIR))?;

        Ok(PkgarBackend {
            packages,
            repo_manager,
            pkey_file: None,
        })
    }

    fn get_pkey(&mut self) -> Result<&PublicKeyFile, Error> {
        if self.pkey_file.is_none() {
            fs::create_dir_all("/tmp/pkg/")?;
            self.repo_manager.download_backend.download(
                "https://static.redox-os.org/pkg/id_ed25519.pub.toml",
                Path::new("/tmp/pkg/pub_key.toml"),
                &mut NoCallback {},
            )?;

            self.pkey_file = Some(PublicKeyFile::open("/tmp/pkg/pub_key.toml")?);
        }

        Ok(self.pkey_file.as_ref().unwrap())
    }

    fn get_package_head(&mut self, package: &String) -> Result<PackageFile, Error> {
        let path = format!("{}/{}/{package}.pkgar_head", INSTALL_PATH, PACKAGES_DIR);

        Ok(PackageFile::new(path, &self.get_pkey()?.pkey)?)
    }

    fn get_package(&mut self, package: &String) -> Result<PackageFile, Error> {
        Ok(PackageFile::new(
            format!("{}/{package}.pkgar", DOWNLOAD_PATH),
            &self.get_pkey()?.pkey,
        )?)
    }

    fn remove_package_head(&mut self, package: &String) -> Result<(), Error> {
        let path = format!("{}/{}/{package}.pkgar_head", INSTALL_PATH, PACKAGES_DIR);

        fs::remove_file(path)?;
        Ok(())
    }

    fn create_head(&mut self, package: &String) -> Result<(), Error> {
        // creates a head file
        pkgar::split(
            "/tmp/pkg/pub_key.toml",
            format!("{}/{package}.pkgar", DOWNLOAD_PATH),
            format!("{}/{}/{package}.pkgar_head", INSTALL_PATH, PACKAGES_DIR),
            Option::<&str>::None,
        )?;

        Ok(())
    }
}

impl Backend for PkgarBackend {
    fn install(
        &mut self,
        package: String,
        callback: &mut dyn crate::Callback,
    ) -> Result<(), Error> {
        self.repo_manager
            .sync(&format!("{package}.pkgar"), callback)?;

        let mut pkg = self.get_package(&package)?;

        let mut install = Transaction::install(&mut pkg, INSTALL_PATH)?;
        install.commit()?;

        self.create_head(&package)?;

        Ok(())
    }

    fn uninstall(&mut self, package: String) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Err(Error::ProtectedPackage(package));
        }

        let mut pkg = self.get_package_head(&package)?;
        let mut remove = Transaction::remove(&mut pkg, INSTALL_PATH)?;
        remove.commit()?;

        self.remove_package_head(&package)?;

        Ok(())
    }

    fn upgrade(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error> {
        let mut pkg = self.get_package_head(&package)?;

        self.repo_manager
            .sync(&format!("{package}.pkgar"), callback)?;
        let mut pkg2 = self.get_package(&package)?;

        let mut update = Transaction::replace(&mut pkg, &mut pkg2, INSTALL_PATH)?;
        update.commit()?;

        self.create_head(&package)?;

        Ok(())
    }

    fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        let entries = fs::read_dir(format!("{}/{}", INSTALL_PATH, PACKAGES_DIR))?;

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
                packages.push(package);
            }
        }

        Ok(packages)
    }
}

impl Drop for PkgarBackend {
    fn drop(&mut self) {
        let packages_path = format!("{}/{}", INSTALL_PATH, PACKAGES_PATH);
        fs::write(packages_path, self.packages.to_toml()).unwrap();
    }
}
