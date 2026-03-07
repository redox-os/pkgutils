use std::collections::{btree_map, BTreeMap};
use std::{cell::RefCell, cmp::Ordering, path::Path, rc::Rc};

use crate::backend::pkgar_backend::PkgarBackend;
use crate::backend::{Backend, Error};
use crate::net_backend::{DefaultNetBackend, DownloadBackend};
use crate::repo_manager::RepoManager;

use crate::callback::Callback;
use crate::package::{PackageInfo, PackageName, RemotePackage};

use crate::{sorensen, PackageState};

pub struct Library {
    /// the computed package state before commit
    package_state: PackageState,
    cached_info: BTreeMap<PackageName, RemotePackage>,
    backend: Box<dyn Backend>,
    callback: Rc<RefCell<dyn Callback>>,
}

impl Library {
    /// Create standard network-based package library from existing configuration on install_path
    pub fn new<P: AsRef<Path>>(
        install_path: P,
        target: &str,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let download_backend = DefaultNetBackend::new()?;

        let mut repo_manager = RepoManager::new(callback.clone(), Box::new(download_backend));
        repo_manager.update_remotes(target, install_path)?;

        let backend = PkgarBackend::new(install_path, repo_manager)?;

        Ok(Library {
            package_state: backend.get_package_state(),
            backend: Box::new(backend),
            cached_info: BTreeMap::new(),
            callback: callback,
        })
    }

    /// Create local-based package library from provided local on install_path
    pub fn new_local<P: AsRef<Path>>(
        source_dir: P,
        pubkey_dir: P,
        install_path: P,
        target: &str,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let download_backend = DefaultNetBackend::new()?;

        let mut repo_manager = RepoManager::new(callback.clone(), Box::new(download_backend));

        repo_manager.add_local(
            "local",
            &source_dir.as_ref().to_string_lossy(),
            target,
            pubkey_dir.as_ref(),
        )?;

        let backend = PkgarBackend::new(install_path, repo_manager)?;

        Ok(Library {
            package_state: backend.get_package_state(),
            backend: Box::new(backend),
            cached_info: BTreeMap::new(),
            callback: callback,
        })
    }

    /// Create remote-based package library from provided list of remote_urls
    pub fn new_remote<P: AsRef<Path>>(
        remote_urls: &Vec<&str>,
        install_path: P,
        target: &str,
        callback: Rc<RefCell<dyn Callback>>,
    ) -> Result<Self, Error> {
        let install_path = install_path.as_ref();

        let download_backend = DefaultNetBackend::new()?;

        let mut repo_manager = RepoManager::new(callback.clone(), Box::new(download_backend));

        for remote_url in remote_urls {
            repo_manager.add_remote(remote_url.trim(), target)?;
        }

        let backend = PkgarBackend::new(install_path, repo_manager)?;

        Ok(Library {
            package_state: backend.get_package_state(),
            backend: Box::new(backend),
            cached_info: BTreeMap::new(),
            callback: callback,
        })
    }

    pub fn get_installed_packages(&self) -> Result<Vec<PackageName>, Error> {
        Ok(self.package_state.get_installed_list())
    }

    pub fn install(&mut self, packages: Vec<PackageName>) -> Result<(), Error> {
        self.callback.borrow_mut().fetch_start(packages.len());
        self.install_inner(packages.clone(), 100)?;
        self.package_state.mark_as_manual(true, &packages);
        self.callback.borrow_mut().fetch_end();
        Ok(())
    }

    fn install_inner(&mut self, packages: Vec<PackageName>, iter: u32) -> Result<(), Error> {
        if iter == 0 {
            return Err(Error::RepoRecursion(packages));
        }
        let mut pinfos = Vec::new();
        for p in &packages {
            let premote = match self.cached_info.entry(p.clone()) {
                btree_map::Entry::Occupied(occupied_entry) => occupied_entry.get().clone(),
                btree_map::Entry::Vacant(vacant_entry) => {
                    let p = self.backend.get_package_detail(p)?;
                    vacant_entry.insert(p).clone()
                }
            };
            self.callback.borrow_mut().fetch_package_increment(1, 0);
            pinfos.push(premote);
        }
        let remainder = self.package_state.install(&pinfos);
        if remainder.len() > 0 {
            self.callback
                .borrow_mut()
                .fetch_package_increment(0, remainder.len());
            self.install_inner(remainder, iter - 1)?;
        }
        Ok(())
    }

    pub fn uninstall(&mut self, packages: Vec<PackageName>) -> Result<(), Error> {
        self.uninstall_inner(packages, 100)
    }

    fn uninstall_inner(&mut self, packages: Vec<PackageName>, iter: u32) -> Result<(), Error> {
        if iter == 0 {
            return Err(Error::RepoRecursion(packages));
        }
        let remainder = self.package_state.uninstall(&packages);
        if remainder.len() > 0 {
            self.uninstall_inner(remainder, iter - 1)?;
        }
        Ok(())
    }

    /// if packages is empty then update all installed packages
    pub fn update(&mut self, mut packages: Vec<PackageName>) -> Result<(), Error> {
        let repo_list = self.backend.get_repository_detail()?;
        let local_list = self.backend.get_package_state();
        if packages.len() == 0 {
            packages = local_list.get_installed_list();
        }

        let mut new_packages = Vec::new();
        for package in packages {
            if let Some(source_hash) = repo_list.packages.get(package.as_str()) {
                if let Some(local_hash) = local_list.installed.get(package.as_str()) {
                    if local_hash.blake3 != *source_hash {
                        new_packages.push(package);
                    }
                }
            }
        }

        self.install(new_packages)
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

    pub fn abort(&mut self) -> Result<usize, Error> {
        self.backend.abort_state()
    }

    pub fn apply(&mut self) -> Result<usize, Error> {
        self.apply_inner()
    }

    fn apply_inner(&mut self) -> Result<usize, Error> {
        let diff = self.backend.get_package_state().diff(&self.package_state);
        if diff.is_empty() {
            return Ok(0);
        }

        self.callback.borrow_mut().install_prompt(&diff)?;

        for package in &diff.uninstall {
            // TODO: Allow self-trusting the package?
            let r = self.backend.uninstall(package.clone());
            if let Err(Error::RepoCacheNotFound(e)) = &r {
                eprintln!("Repository source of {e} is not valid, please reinstall repository public keys to allow erasing, or reinstall the package.");
            }
            r?
        }

        for package in &diff.update {
            if let Some(cache) = self.cached_info.remove(package) {
                let r = self.backend.upgrade(&cache);
                if let Err(Error::RepoCacheNotFound(e)) = &r {
                    eprintln!("Repository source of {e} is not valid, reinstalling!");
                    self.backend.install(cache)?;
                }
                r?
            }
        }

        for package in &diff.install {
            if let Some(cache) = self.cached_info.remove(package) {
                self.backend.install(cache)?;
            }
        }

        self.callback
            .borrow_mut()
            .install_check_conflict(self.backend.commit_check_conflict()?)?;

        self.backend.commit_state(self.package_state.clone())
    }

    pub fn info(&mut self, package: PackageName) -> Result<PackageInfo, Error> {
        let installed = self.package_state.get_installed_list().contains(&package);
        let package = self.backend.get_package_detail(&package)?;

        Ok(PackageInfo { installed, package })
    }
}
