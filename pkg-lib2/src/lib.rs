use std::{
    cmp::Ordering,
    fs::{self, File},
    io::{self, Read},
    path::Path, env,
};

use backend::{request::ReqwestBackend, Backend, DownloadBackend, DownloadError, tar::TarBackend, pkgar::PkgarBackaend};
use package::Package;
use package_list::*;

mod backend;
mod package;
mod package_list;
mod sorensen;

const PACKAGES_PATH: &str = "/tmp/pkg/packages.toml";
const REPOS_PATH: &str = "/etc/pkg/repos";

pub struct Repo {
    pub remotes: Vec<String>,
    pub target: String,
    pub install_path: String,
    pub download_path: String,

    pub package_list: PackageList,
    backend: Box<dyn Backend>,
    download_backend: Box<dyn DownloadBackend>,
}

#[derive(Debug)]
pub struct PackageInfo {
    pub installed: bool,
    pub version: String,
    pub target: String,

    pub download_size: String,
    pub install_size: String,

    pub checksum: String,
    pub depends: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    TomlRead(toml::de::Error),
    DownloadError(DownloadError),
    GetingPackgeFaild(Box<Error>),
    PackageNotFound(String),
    PathIsNotValidUnitcode(String),
    PkgarError(Box<pkgar::Error>),
    PkgarKeysError(pkgar_keys::Error),
}
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IO(value)
    }
}
impl From<toml::de::Error> for Error {
    fn from(value: toml::de::Error) -> Self {
        Error::TomlRead(value)
    }
}
impl From<DownloadError> for Error {
    fn from(value: DownloadError) -> Self {
        Error::DownloadError(value)
    }
}
impl From<pkgar::Error> for Error {
    fn from(value: pkgar::Error) -> Self {
        Error::PkgarError(Box::new(value))
    }
}
impl From<pkgar_keys::Error> for Error {
    fn from(value: pkgar_keys::Error) -> Self {
        Error::PkgarKeysError(value)
    }
}

impl Repo {
    pub fn new(target: &str, install_path: &str, download_path: &str) -> Result<Self, Error> {
        if !Path::new(&install_path).exists() {
            fs::create_dir(install_path)?;
        }
        if !Path::new(&download_path).exists() {
            fs::create_dir(download_path)?;
        }

        let mut remotes = vec![];
        remotes.push("https://static.redox-os.org/pkg".to_string());

        if let Ok(mut file) = File::open(REPOS_PATH) {
            let mut data = String::new();
            if file.read_to_string(&mut data).is_ok() {
                for line in data.lines() {
                    if !line.starts_with('#') {
                        remotes.push(line.to_string());
                    }
                }
            }
        }
        let download_backend = ReqwestBackend {};

        let backend: Box<dyn Backend> = if env::var("USE_PKGAR").is_ok() {
            Box::new(PkgarBackaend::new(
                target,
                install_path,
                download_path,
                Box::new(download_backend),
            )?)
        } else {
            Box::new(TarBackend::new(target, install_path, download_path, Box::new(download_backend))?)
        };

        Ok(Repo {
            package_list: Default::default(),
            backend,
            download_backend: Box::new(download_backend),
            remotes,
            target: target.to_string(),
            install_path: install_path.to_string(),
            download_path: download_path.to_string(),
        })
    }

    fn get_all_package_names(&self) -> Result<Vec<String>, Error> {
        // get website html
        let local_path = format!("{}/website", self.download_path);

        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut names = vec![];
        for remote in &self.remotes {
            let remote_path = format!("{}/{}/", remote, self.target);
            self.download_backend.download(&remote_path, &local_path)?;

            let mut file = File::open(&local_path)?;
            let mut string = String::new();
            file.read_to_string(&mut string)?;

            let mut response = string;

            while let Some(end) = response.find(".toml</a>") { 
                let mut i = end;
                loop {
                    let char = response.chars().nth(i).expect("this should work");
                    if char == '>' {
                        break;
                    }
                    i -= 1;
                }
                let package_name = &response[i + 1..end];
                if !names.contains(&package_name.to_string()) {
                    names.push(package_name.to_string());
                }

                response = response.replacen(".toml</a>", "", 1);
            }
        }

        Ok(names)
    }

    pub fn install(&mut self, packages: Vec<String>) -> Result<(), Error> {
        for package_name in packages {
            self.package_list.install.push(package_name);
        }

        Ok(())
    }

    pub fn uninstall(&mut self, packages: Vec<String>) -> Result<(), Error> {
        for package_name in packages {
            self.package_list.uninstall.push(package_name);
        }

        Ok(())
    }

    /// if packages is empty then update all installed packages
    pub fn update(&mut self, packages: Vec<String>) -> Result<(), Error> {
        if packages.is_empty() {
            for package_name in &self.backend.get_installed_packages()? {
                self.package_list.install.push(package_name.to_string());
            }
        } else {
            for package_name in packages {
                self.package_list.install.push(package_name);
            }
        }

        Ok(())
    }

    pub fn search(&self, package: &str) -> Result<Vec<String>, Error> {
        let names = self.get_all_package_names()?;

        let mut result = vec![];

        for name in names {
            let mut rank = 0;

            /*if name.contains(package) {
                rank += 1;
            }

            if name.starts_with(package) {
                rank += 1;
            }*/

            let dst = sorensen::distance(
                package.to_lowercase().as_bytes(),
                name.to_lowercase().as_bytes(),
            );
            if dst >= 0.2 {
                rank += (dst * 100.0) as i32;
                //println!("{dst}");
            }

            if rank > 0 {
                result.push((name, rank));
            }
        }

        result.as_mut_slice().sort_by(|a, b| {
            let check1 = b.1.cmp(&a.1);
            if check1 == Ordering::Equal {
                a.0.cmp(&b.0)
            } else {
                check1
            }
        });

        Ok(result.into_iter().map(|x| x.0).collect())
    }

    pub fn apply(&mut self, callback: &mut dyn Callback) -> Result<(), Error> {
        for package in &self.package_list.uninstall {
            self.backend.uninstall(package.to_string())?;
        }

        let install = self.get_dependecies(&self.package_list.install)?;

        for package in install {
            if self.backend.get_installed_packages()?.contains(&package) {
                self.backend.upgrade(package, callback)?;
            } else {
                self.backend.install(package, callback)?;
            }
        }

        self.package_list = Default::default();
        Ok(())
    }

    pub fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        self.backend.get_installed_packages()
    }

    fn sync(&self, file: &str) -> Result<(), DownloadError> {
        let local_path = format!("{}/{}", self.download_path, file);

        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut res = Err(DownloadError::NoReposWereAdded);
        for remote in self.remotes.iter() {
            let remote_path = format!("{}/{}/{}", remote, self.target, file);
            res = self.download_backend.download(&remote_path, &local_path);
            if res.is_ok() {
                break;
            }
        }

        res
    }

    pub fn get_package(&self, package_name: &str) -> Result<Package, Error> {
        self.sync(&format!("{package_name}.toml"))
            .map_err(|_| Error::PackageNotFound(package_name.to_owned()))?;

        let local_path = format!("{}/{package_name}.toml", self.download_path);

        let mut file = File::open(local_path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;

        Ok(Package::from_toml(&toml)?)
    }

    fn get_dependecies(&self, packages: &Vec<String>) -> Result<Vec<String>, Error> {
        let mut list = vec![];
        for package in packages {
            self.get_dependecies_recursive(package, &mut list)?;
        }

        Ok(list)
    }

    fn get_dependecies_recursive(
        &self,
        package_name: &str,
        list: &mut Vec<String>,
    ) -> Result<(), Error> {
        let package = self.get_package(package_name)?;
        for dep in &package.depends {
            let package = self.get_package(dep)?;

            if list.contains(&package.name) {
                continue;
            }

            list.push(package.name.clone());
            self.get_dependecies_recursive(package_name, list)?;
        }
        list.push(package.name);
        Ok(())
    }

    pub fn info(&self, package: String) -> Result<PackageInfo, Error> {
        self.sync(&format!("{}.sig", package))?;
        let sig = fs::read_to_string(format!("{}/{}.sig", &self.download_path, package))?;

        let installed = self.backend.get_installed_packages()?.contains(&package);
        let package = self.get_package(&package)?;

        Ok(PackageInfo {
            installed,
            version: package.version,
            target: package.target,
            download_size: "not implemented".to_string(),
            install_size: "not implemented".to_string(),
            checksum: sig,
            depends: package.depends,
        })
    }
}

pub trait Callback {
    fn start(&mut self, length: u64, file: &str);
    fn update(&mut self, downloaded: u64);
    fn end(&mut self);
}
