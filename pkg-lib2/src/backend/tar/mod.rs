use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
};

use libflate::gzip::Decoder;
use tar::Archive;
use crate::{Callback, Error, PACKAGES_PATH, REPOS_PATH};
use self::files::Files;
use super::{Backend, DownloadBackend, DownloadError};

mod files;

pub struct TarBackend {
    pub remotes: Vec<String>,
    pub target: String,
    pub install_path: String,
    pub download_path: String,

    download_backend: Box<dyn DownloadBackend>,
    packages: Files,
}

impl TarBackend {
    #[allow(dead_code)]
    pub fn new(
        target: &str,
        install_path: &str,
        download_path: &str,
        download_backend: Box<dyn DownloadBackend>,
    ) -> Result<Self, Error> {
        let mut files_string = String::new();
        let packages;
        let packages_path = format!("{}/{}", install_path, PACKAGES_PATH);

        match File::open(&packages_path) {
            Ok(mut file) => {
                file.read_to_string(&mut files_string)?;
                packages = Files::from_toml(&files_string)?;
            }
            Err(_) => {
                packages = Default::default();
                fs::write(PACKAGES_PATH, packages.to_toml()).unwrap();
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

        Ok(TarBackend {
            packages,
            remotes,
            target: target.to_owned(),
            install_path: install_path.to_owned(),
            download_path: download_path.to_owned(),
            download_backend,
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

    fn uninstall_package(&mut self, package: &str) -> Result<(), Error> {
        if self.packages.files.contains_key(package) {
            let paths = &self.packages.files[package];

            for path in paths {
                let mut path_is_used = false;

                for (package2, paths2) in &self.packages.files {
                    if package2 != package {
                        for path2 in paths2 {
                            if path == path2 {
                                path_is_used = true;
                            }
                        }
                    }
                }

                if path_is_used {
                    continue;
                }

                let install_path = Path::new(&self.install_path);
                let path = install_path.join(path);

                if path.is_dir() {
                    fs::remove_dir_all(path)?;
                } else if path.is_file() {
                    fs::remove_file(path)?;
                }
            }
        }

        self.packages.files.remove(package);
        self.packages.installed.remove(package);
        Ok(())
    }
}

impl Backend for TarBackend {
    fn install(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error> {
        self.sync_with_callback(&format!("{}.tar.gz", package), callback)?;
        let path = format!("{}/{}.tar.gz", self.download_path, package);
        let file = File::open(&path)?;
        let decoder = Decoder::new(BufReader::new(file))?;

        let mut ar = Archive::new(decoder);
        ar.set_preserve_permissions(true);

        if !self.packages.files.contains_key(&package) {
            self.packages.files.insert(package.clone(), vec![]);
        }

        let files = self.packages.files.get_mut(&package).unwrap(); // never fails
        for entry in ar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            let file = path
                .to_str()
                .ok_or(Error::PathIsNotValidUnitcode(package.clone()))?
                .to_string();

            if !files.contains(&file) {
                files.push(file);
            }

            entry.unpack_in(self.install_path.clone())?;
        }

        self.sync_with_callback(&format!("{}.sig", package), callback)?;
        let sig = fs::read_to_string(format!("{}/{}.sig", &self.download_path, package))?;
        self.packages.installed.insert(package, sig);

        Ok(())
    }

    fn uninstall(&mut self, package: String) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Ok(());
        }
        self.uninstall_package(&package)?;

        Ok(())
    }

    fn upgrade(&mut self, package: String, callback: &mut dyn Callback) -> Result<(), Error> {
        self.sync_with_callback(&format!("{}.sig", package), callback)?;
        let sig = fs::read_to_string(format!("{}/{}.sig", &self.download_path, package))?;

        if self.packages.installed[&package] == sig {
            return Ok(());
        }

        self.uninstall_package(&package)?;

        self.install(package, callback)?;

        Ok(())
    }

    fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .packages
            .installed.keys().map(|x| x.to_string())
            .collect())
    }
}

impl Drop for TarBackend {
    fn drop(&mut self) {
        fs::write(PACKAGES_PATH, self.packages.to_toml()).unwrap();
    }
}
