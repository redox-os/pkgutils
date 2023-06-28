use std::{cmp::Ordering, fs};

use backend::{Error, Backend, Callback, pkgar_backend::PkgarBackend, reqwest_backend::ReqwestBackend};
use package::{Package, PackageInfo};
use package_list::PackageList;
use repo_manager::RepoManager;

mod sorensen;
mod package;
mod package_list;
pub mod backend;
mod repo_manager;

pub struct Library<'a> { 
    pub package_list: PackageList,
    repo_manager: RepoManager,
    backend: Box<dyn Backend>,
    callback: &'a mut dyn Callback,
}

const DOWNLOAD_PATH: &str = "/tmp/pkg_dowload/";
const INSTALL_PATH: &str = "/tmp/pkg_install/";
const REPOS_PATH: &str = "/tmp/pkg_install/etc/pkg/repos";
const PACKAGES_PATH: &str = "etc/pkg/packages.toml";


impl<'a> Library<'a> {
    pub fn new(callback: &'a mut dyn Callback) -> Result<Self, Error>{

        let mut remotes = vec![];
        remotes.push("https://static.redox-os.org/pkg/x86_64-unknown-redox".to_string());

        let repos_path = format!("{}/{}", INSTALL_PATH, REPOS_PATH);
        let file = fs::read_to_string(repos_path);

        if let Ok(data) = file {
            for line in data.lines() {
                if !line.starts_with('#') {
                    remotes.push(line.to_string());
                }
            }
        }

        let download_backend = ReqwestBackend {};

        let repo_manager = RepoManager {
            remotes: remotes.clone(),
            download_path: DOWNLOAD_PATH.into(),
            download_backend: Box::new(download_backend),
        };
        
        let backend = PkgarBackend::new(repo_manager)?;

        let repo_manager = RepoManager {
            remotes,
            download_path: DOWNLOAD_PATH.into(),
            download_backend: Box::new(download_backend),
        };

        Ok(Library {  
            repo_manager,
            package_list: PackageList::default(),
            backend: Box::new(backend),
            callback
        })
    }

    pub fn get_installed_packages(&self) -> Result<Vec<String>, Error> {
        self.backend.get_installed_packages()
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

    fn get_all_package_names(&mut self) -> Result<Vec<String>, Error> {
        // get website html
        self.repo_manager.sync("", self.callback)?;
        let mut website = fs::read_to_string(self.repo_manager.download_path.join("website"))?;

        let mut names = vec![];
        while let Some(end) = website.find(".toml</a>") { 
            let mut i = end;
            loop {
                let char = website.chars().nth(i).expect("this should work");
                if char == '>' {
                    break;
                }
                i -= 1;
            }
            let package_name = &website[i + 1..end];
            if !names.contains(&package_name.to_string()) {
                names.push(package_name.to_string());
            }

            website = website.replacen(".toml</a>", "", 1);
        }

        Ok(names)
    }

    pub fn search(&mut self, package: &str) -> Result<Vec<(String, f64)>, Error> {
        let names = self.get_all_package_names()?;

        let mut result = vec![];

        for name in names {
            let mut rank = 0.0;

            let dst = sorensen::distance(
                package.to_lowercase().as_bytes(),
                name.to_lowercase().as_bytes(),
            );
            if dst >= 0.2 {
                rank += dst;
                //println!("{dst}");
            }

            if name.contains(package) {
                rank += 0.01;
            }

            if rank > 0.0 {
                result.push((name, rank));
            }
        }

        result.as_mut_slice().sort_by(|a, b| {
            let check1 = b.1.partial_cmp(&a.1);
            if check1 == Some(Ordering::Equal) {
                a.0.cmp(&b.0)
            } else {
                check1.unwrap_or(Ordering::Equal)
            }
        });

        Ok(result)
    }

    pub fn apply(&mut self) -> Result<(), Error> {

        for package in self.package_list.uninstall.iter() {
            self.backend.uninstall(package.to_string())?;
        }

        let install = self.get_dependecies(&self.package_list.install.clone())?;

        for package in install.iter() {
            if self.backend.get_installed_packages()?.contains(package) {
                self.backend.upgrade(package.into(), self.callback)?;
            } else {
                self.backend.install(package.into(), self.callback)?;
            }
        }

        self.package_list = Default::default();
        Ok(())
    }

    fn get_package(&mut self, package_name: &str) -> Result<Package, Error> {

        let toml = self.repo_manager.sync_and_read(&format!("{package_name}.toml"), self.callback)
            .map_err(|_| Error::PackageNotFound(package_name.to_owned()))?;

        Ok(Package::from_toml(&toml)?)
    }

    pub fn get_dependecies(&mut self, packages: &Vec<String>) -> Result<Vec<String>, Error> {
        let mut list = vec![];
        for package in packages {
            self.get_dependecies_recursive(package, &mut list)?;
        }

        Ok(list)
    }

    fn get_dependecies_recursive(
        &mut self,
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

    pub fn info(&mut self, package: String) -> Result<PackageInfo, Error> {
        let sig = self.repo_manager.sync_and_read(&format!("{}.sig", package), self.callback)?;

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

