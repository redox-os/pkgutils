use std::{
    fs::{self, File},
    io::{BufReader, ErrorKind},
    path::Path,
};

use self::files::Packages;
use super::Backend;
use crate::{repo_manager::RepoManager, Error, DOWNLOAD_PATH, INSTALL_PATH, PACKAGES_PATH};
use libflate::gzip::Decoder;
use tar::Archive;

mod files;

pub struct TarBackend {
    repo_manager: RepoManager,
    packages: Packages,
}

impl TarBackend {
    #[allow(dead_code)]
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

        Ok(TarBackend {
            packages,
            repo_manager,
        })
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

                let path = Path::new(INSTALL_PATH).join(path);

                if path.is_dir() {
                    match fs::remove_dir(path) {
                        Ok(_) => {}
                        //#[cfg(target_os = "redox")]
                        //Err(e) => { if e.kind() == ErrorKind::DirectoryNotEmpty { println!("{path:?} is not empty") } },
                        Err(_) => {}
                    }
                    //fs::remove_dir_all(path)?;
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
    fn install(&mut self, package: String) -> Result<(), Error> {
        self.repo_manager.sync_tar(&package);
        let path = format!("{}/{}.tar.gz", DOWNLOAD_PATH, package);
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

            entry.unpack_in(INSTALL_PATH)?;
        }

        let sig = self.repo_manager.sync_sig(&package);
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

    fn upgrade(&mut self, package: String) -> Result<(), Error> {
        let sig = self.repo_manager.sync_sig(&package);

        if self.packages.installed[&package] == sig {
            return Ok(());
        }

        self.uninstall_package(&package)?;

        self.install(package)?;

        Ok(())
    }

    fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .packages
            .installed
            .keys()
            .map(|x| x.to_string())
            .collect())
    }
}

impl Drop for TarBackend {
    fn drop(&mut self) {
        let packages_path = format!("{}/{}", INSTALL_PATH, PACKAGES_PATH);
        fs::write(packages_path, self.packages.to_toml()).unwrap();
    }
}
