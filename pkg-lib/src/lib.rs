//#![cfg_attr(target_os = "redox", feature(io_error_more))]

use std::{
    cell::RefCell,
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use backend::pkgar_backend::PkgarBackend;
use backend::Backend;
use net_backend::{DefaultNetBackend, DownloadBackend};
use package_list::PackageList;
use repo_manager::RepoManager;

pub use backend::Error;
pub use callback::Callback;
pub use package::{Package, PackageInfo, PackageName};

pub mod backend;
pub mod callback;
pub mod net_backend;
pub mod package;
pub mod recipes;

mod package_list;
mod repo_manager;
mod sorensen;

pub struct Library {
    package_list: PackageList,
    backend: Box<dyn Backend>,
}

const DOWNLOAD_PATH: &str = "/tmp/pkg_download/";

// make them not relative
const PACKAGES_PATH: &str = "etc/pkg/packages.toml";

impl Library {
    pub fn new<P: AsRef<Path>>(
        install_path: P,
        target: &str,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let download_backend = DefaultNetBackend::new()?;
        let prefer_cache = PathBuf::from(DOWNLOAD_PATH).join("prefer_cache").exists();

        let mut repo_manager = RepoManager {
            remotes: Vec::new(),
            download_path: DOWNLOAD_PATH.into(),
            download_backend: Box::new(download_backend.clone()),
            callback: callback.clone(),
            prefer_cache,
        };

        {
            let repos_path = install_path.join("etc/pkg.d");
            let mut repo_files = Vec::new();
            for entry_res in fs::read_dir(&repos_path)? {
                let entry = entry_res?;
                let path = entry.path();
                if path.is_file() {
                    repo_files.push(path);
                }
            }
            repo_files.sort();
            for repo_file in repo_files {
                let data = fs::read_to_string(repo_file)?;
                for line in data.lines() {
                    if !line.starts_with('#') {
                        repo_manager.add_remote(line.trim(), target)?;
                    }
                }
            }
        }

        let backend = PkgarBackend::new(install_path, repo_manager)?;

        Ok(Library {
            package_list: PackageList::default(),
            backend: Box::new(backend),
        })
    }

    pub fn get_installed_packages(&self) -> Result<Vec<PackageName>, Error> {
        self.backend.get_installed_packages()
    }

    pub fn install(&mut self, packages: Vec<PackageName>) -> Result<(), Error> {
        let installed_packages = self.get_installed_packages().unwrap_or(vec![]);
        for package_name in packages {
            if !installed_packages.contains(&package_name) {
                self.package_list.install.push(package_name);
            }
        }

        Ok(())
    }

    /// TODO: Cannot uninstall depedencies as manual mark is not implemented
    pub fn uninstall(&mut self, packages: Vec<PackageName>) -> Result<(), Error> {
        let installed_packages = self.get_installed_packages()?;
        for package_name in packages {
            if installed_packages.contains(&package_name) {
                self.package_list.uninstall.push(package_name);
            }
        }

        Ok(())
    }

    /// if packages is empty then update all installed packages
    pub fn update(&mut self, packages: Vec<PackageName>) -> Result<(), Error> {
        let installed_packages = self.get_installed_packages()?;
        if packages.is_empty() {
            for package_name in &installed_packages {
                self.package_list.install.push(package_name.clone());
            }
        } else {
            for package_name in packages {
                if installed_packages.contains(&package_name) {
                    self.package_list.install.push(package_name);
                }
            }
        }

        Ok(())
    }

    pub fn get_all_package_names(&mut self) -> Result<Vec<PackageName>, Error> {
        let repository = self.backend.get_repository_detail()?;
        let list = repository
            .packages
            .keys()
            .cloned()
            .fold(Vec::new(), |mut acc, x| {
                match PackageName::new(x) {
                    Ok(name) => {
                        acc.push(name);
                    }
                    Err(_) => {}
                };
                acc
            });
        Ok(list)
    }

    pub fn search(&mut self, package: &str) -> Result<Vec<(PackageName, f64)>, Error> {
        let names = self.get_all_package_names()?;

        let mut result = vec![];

        for name in names {
            let mut rank = 0.0;

            let dst = sorensen::distance(
                package.to_lowercase().as_bytes(),
                name.as_str().to_lowercase().as_bytes(),
            );

            if dst >= 0.2 {
                rank += dst;
            }

            if name.as_str().contains(package) {
                rank += 0.01;
            }

            if rank > 0.0 {
                result.push((name, rank));
            }
        }

        // this is hard to read
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
            self.backend.uninstall(package.clone())?;
        }

        let install = self.with_dependecies(&self.package_list.install.clone())?;

        for package in install.into_iter() {
            if self.backend.get_installed_packages()?.contains(&package) {
                self.backend.upgrade(package)?;
            } else {
                self.backend.install(package)?;
            }
        }

        self.package_list = Default::default();
        Ok(())
    }

    pub fn with_dependecies(
        &mut self,
        packages: &Vec<PackageName>,
    ) -> Result<Vec<PackageName>, Error> {
        let mut list = vec![];
        for package in packages {
            self.get_dependecies_recursive(package, &mut list)?;
        }

        Ok(list)
    }

    fn get_dependecies_recursive(
        &mut self,
        package_name: &PackageName,
        list: &mut Vec<PackageName>,
    ) -> Result<(), Error> {
        let package = self.backend.get_package_detail(package_name)?;
        if list.contains(&package.name) {
            return Ok(());
        }
        for dep in &package.depends {
            self.get_dependecies_recursive(dep, list)?;
        }

        // meta-packages is identified with empty version
        // TODO: This is not the right time to check it,
        // but the TOML data we needed will lost outside this fn
        if package.version != "" {
            list.push(package.name);
        }
        Ok(())
    }

    pub fn info(&mut self, package: PackageName) -> Result<PackageInfo, Error> {
        let installed = self.backend.get_installed_packages()?.contains(&package);
        let package = self.backend.get_package_detail(&package)?;

        Ok(PackageInfo { installed, package })
    }
}
