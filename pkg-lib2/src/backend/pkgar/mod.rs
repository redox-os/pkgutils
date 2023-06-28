mod packages;

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::PublicKeyFile;

use crate::{Callback, Error, PACKAGES_PATH, REPOS_PATH};
use self::packages::Packages;
use super::{Backend, DownloadBackend, DownloadError};

pub struct PkgarBackaend {
    pub remotes: Vec<String>,
    pub target: String,
    pub install_path: String,
    pub download_path: String,

    packages: Packages,
    download_backend: Box<dyn DownloadBackend>,
    pkey_file: PublicKeyFile,
}

impl PkgarBackaend {
    #[allow(dead_code)]
    pub fn new(
        target: &str,
        install_path: &str,
        download_path: &str,
        download_backend: Box<dyn DownloadBackend>,
    ) -> Result<Self, Error> {
        let packages;

        let packages_path = format!("{}/{}", install_path, PACKAGES_PATH);
        match File::open(&packages_path) {
            Ok(mut file) => {
                let mut files_string = String::new();
                file.read_to_string(&mut files_string)?;
                packages = Packages::from_toml(&files_string)?;
            }
            Err(_) => {
                packages = Default::default();
                fs::write(packages_path, packages.to_toml())?;
            }
        };

        let mut remotes = vec![];
        remotes.push("https://static.redox-os.org/pkg".to_string());

        let repos_path = format!("{}/{}", install_path, REPOS_PATH);
        if let Ok(mut file) = File::open(repos_path) {
            let mut data = String::new();
            if file.read_to_string(&mut data).is_ok() {
                for line in data.lines() {
                    if !line.starts_with('#') {
                        remotes.push(line.to_string());
                    }
                }
            }
        }

        fs::create_dir_all(format!("{}/etc/pkg/packages/", install_path))?;

        download_backend.download(
            "https://static.redox-os.org/pkg/id_ed25519.pub.toml",
            "/tmp/pkg/pub_key.toml",
        )?;

        let pkey_file = PublicKeyFile::open("/tmp/pkg/pub_key.toml")?;

        Ok(PkgarBackaend {
            remotes,
            target: target.to_owned(),
            install_path: install_path.to_owned(),
            download_path: download_path.to_owned(),

            packages,
            download_backend,
            pkey_file,
        })
    }

    fn sync_with_callback(
        &self,
        file: &str,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        let local_path = format!("{}/{}", self.download_path, file);

        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut res = Err(DownloadError::NoReposWereAdded);
        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}/{}", remote, self.target, file);
            res = self
                .download_backend
                .download_with_callback(&remote_path, &local_path, callback);
            if res.is_ok() {
                break;
            }
        }

        res
    }
}

impl Backend for PkgarBackaend {
    fn install(
        &mut self,
        package: String,
        callback: &mut dyn crate::Callback,
    ) -> Result<(), Error> {
        self.sync_with_callback(&format!("{package}.pkgar"), callback)?;

        let mut pkg = PackageFile::new(
            format!("{}/{package}.pkgar", self.download_path),
            &self.pkey_file.pkey,
        )?;

        let mut install = Transaction::install(&mut pkg, self.install_path.clone())?;
        install.commit()?;

        // creates a head file
        pkgar::split(
            "/tmp/pkg/pub_key.toml",
            format!("{}/{package}.pkgar", self.download_path),
            format!(
                "{}/etc/pkg/packages/{package}.pkgar_head",
                self.install_path
            ),
            Option::<&str>::None,
        )?;

        Ok(())
    }

    fn uninstall(&mut self, package: String) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Ok(());
        }

        let path = format!(
            "{}/etc/pkg/packages/{package}.pkgar_head",
            self.install_path
        );

        let mut pkg = PackageFile::new(path.clone(), &self.pkey_file.pkey)?;
        let mut remove = Transaction::remove(&mut pkg, self.install_path.clone())?;
        remove.commit()?;

        fs::remove_file(path)?;

        Ok(())
    }

    fn upgrade(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error> {
        let path = format!(
            "{}/etc/pkg/packages/{package}.pkgar_head",
            self.install_path
        );
        let mut pkg = PackageFile::new(path.clone(), &self.pkey_file.pkey)?;

        self.sync_with_callback(&format!("{package}.pkgar"), callback)?;
        let mut pkg2 = PackageFile::new(
            format!("{}/{package}.pkgar", self.download_path),
            &self.pkey_file.pkey,
        )?;

        let mut update = Transaction::replace(&mut pkg, &mut pkg2, self.install_path.clone())?;
        update.commit()?;

        // creates a head file
        pkgar::split(
            "/tmp/pkg/pub_key.toml",
            format!("{}/{package}.pkgar", self.download_path),
            path,
            Option::<&str>::None,
        )?;

        Ok(())
    }

    fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        let entries = fs::read_dir(format!("{}/etc/pkg/packages/", self.install_path))?;

        let mut packages = vec![];

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_str().ok_or(Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                "file name isn't UTF-8",
            )))?;

            let package = file_name_str.replace(".pkgar", "");
            packages.push(package);
        }

        Ok(packages)
    }
}

impl Drop for PkgarBackaend {
    fn drop(&mut self) {
        fs::write(PACKAGES_PATH, self.packages.to_toml()).unwrap();
    }
}
