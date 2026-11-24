use std::{cell::RefCell, fs, path::Path, rc::Rc};

use pkgar::{PackageFile, Transaction, WorkerPool};
use pkgar_core::PackageSrc;

use super::{Backend, Error};
use crate::{
    Package, PackageName, PathIndex, RemoteName, RemotePath, RepoPublicKeyFile,
    backend::wrap_io_err,
    callback::Callback,
    package::{RemotePackage, Repository},
    package_state::PackageState,
    repo_manager::RepoManager,
};

/// Package backend using pkgar
pub struct PkgarBackend {
    /// Root path, usually "/"
    install_path: PathIndex,
    /// Things in "/etc/pkg/packages.toml"
    packages: PackageState,
    /// Things in "/etc/pkg.d" and inet
    repo_manager: RepoManager,
    /// temporary commit
    commits: Transaction,
    keys_synced: bool,
    skip_local_check: bool,
    threads: usize,
    pool: WorkerPool,
    callback: Rc<RefCell<dyn Callback>>,
}

impl PkgarBackend {
    pub fn new(install_path: PathIndex, mut repo_manager: RepoManager) -> Result<Self, Error> {
        let mut packages = PackageState::from_sysroot(&install_path)?;

        if let Some(target) = repo_manager.machine_target.as_ref()
            && packages.machine_target.is_empty()
        {
            // TODO: CPU feature eligibility
            packages.machine_target = target.to_string()
        }

        // TODO: Use File::lock. This only checks permission
        packages.to_sysroot(&install_path)?;

        fs::create_dir_all(&install_path.var_heads)
            .map_err(wrap_io_err!(&install_path.var_heads, "Creating dir"))?;
        repo_manager.set_download_path(install_path.download_dir.clone());
        let callback = repo_manager.callback.clone();

        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        Ok(PkgarBackend {
            install_path,
            packages,
            repo_manager,
            pool: WorkerPool::new(threads),
            threads,
            // packages_lock,
            commits: Transaction::new(),
            keys_synced: false,
            skip_local_check: false,
            callback,
        })
    }

    // reads /var/lib/packages/[package].pkgar_head
    fn get_package_head(&self, package: &PackageName) -> Result<PackageFile, Error> {
        let path = self
            .install_path
            .var_heads
            .join(format!("{package}.pkgar_head"));

        let Some(pkg) = self.packages.installed.get(package) else {
            return Err(Error::PackageNotInstalled(package.clone()));
        };
        let Some(remote) = self.packages.pubkeys.get(&pkg.remote) else {
            return Err(Error::RepoCacheNotFound(package.clone()));
        };

        let pkg = PackageFile::new(&path, &remote.pkey).map_err(Error::from)?;

        Ok(pkg)
    }

    fn remove_package_head(&mut self, package: &PackageName) -> Result<(), Error> {
        let path = self
            .install_path
            .var_heads
            .join(format!("{package}.pkgar_head"));

        fs::remove_file(&path).map_err(wrap_io_err!(&path, "Removing file"))?;
        Ok(())
    }

    fn create_head(
        &self,
        package_file: &mut PackageFile,
        package: &PackageName,
    ) -> Result<(), Error> {
        // creates a head file
        let head_path = self
            .install_path
            .var_heads
            .join(format!("{package}.pkgar_head"));

        package_file.split(&head_path, None::<&Path>)?;

        Ok(())
    }

    fn sync_keys(&mut self) -> Result<(), Error> {
        if self.keys_synced {
            return Ok(());
        }

        for (name, map) in &mut self.repo_manager.remote_map {
            if map.pubkey.is_none() {
                if let Some(pubk) = self.packages.pubkeys.get(name) {
                    map.pubkey = Some(pubk.pkey)
                }
            }
        }

        self.repo_manager.sync_keys()?;

        self.keys_synced = true;
        Ok(())
    }

    fn install_inner(
        &mut self,
        package: &RemotePackage,
        pkg: &mut PackageFile,
        entries: Vec<pkgar_core::Entry>,
    ) -> Result<(), Error> {
        let mapped: Vec<_> = entries
            .into_iter()
            .filter_map(|e| self.install_path.map_entry(e))
            .collect();

        self.extract_parallel(package, pkg, mapped, |tx, pkg, entry, base, skip| {
            tx.install_one(pkg, entry, base, skip)
                .map_err(|e| Error::Pkgar(Box::new(e)))
        })
    }

    fn extract_parallel<T, Work>(
        &mut self,
        package: &RemotePackage,
        pkg: &mut PackageFile,
        items: Vec<T>,
        work: Work,
    ) -> Result<(), Error>
    where
        T: Send + Sync + 'static,
        Work: Fn(&mut Transaction, &mut PackageFile, &T, &Path, bool) -> Result<(), Error>
            + Send
            + Sync
            + 'static,
    {
        self.callback
            .borrow_mut()
            .extract_start(package, items.len());

        let mut worker_states = Vec::with_capacity(self.threads);
        for _ in 0..self.threads {
            match pkg.reopen().map_err(Error::from) {
                Ok(p) => worker_states.push((p, Transaction::new())),
                Err(e) => {
                    self.callback.borrow_mut().extract_end();
                    return Err(e);
                }
            }
        }

        let base_dir = self.install_path.base_dir.clone();
        let skip_local_check = self.skip_local_check;

        let (states, first_err) = self.pool.run_batch(
            items,
            worker_states,
            move |(worker_pkg, local_tx), item| {
                let before = local_tx.total_indexed();
                work(local_tx, worker_pkg, item, &base_dir, skip_local_check)?;
                Ok(local_tx.total_indexed() - before)
            },
            |delta| {
                self.callback.borrow_mut().extract_increment(delta);
            },
        );

        self.callback.borrow_mut().extract_end();

        for (_, local_tx) in states {
            self.commits.merge(local_tx, Some(pkg));
        }

        if let Some(e) = first_err {
            return Err(e);
        }
        Ok(())
    }

    fn upgrade_inner(
        &mut self,
        package: &RemotePackage,
        pkg: &mut PackageFile,
        entries: Vec<(pkgar_core::Entry, pkgar_core::Entry)>,
    ) -> Result<(), Error> {
        let mapped: Vec<_> = entries
            .into_iter()
            .filter_map(|(old_e, new_e)| {
                Some((
                    self.install_path.map_entry(old_e)?,
                    self.install_path.map_entry(new_e)?,
                ))
            })
            .collect();

        self.extract_parallel(
            package,
            pkg,
            mapped,
            |tx, pkg, (old_e, new_e), base, skip| {
                tx.replace_one(pkg, old_e, new_e, base, skip)
                    .map_err(|e| Error::Pkgar(Box::new(e)))
            },
        )
    }

    fn uninstall_inner(
        &mut self,
        package: &PackageName,
        pkg: &PackageFile,
        entries: Vec<pkgar_core::Entry>,
    ) -> Result<(), Error> {
        let show_progress = !self.skip_local_check;
        if show_progress {
            self.callback
                .borrow_mut()
                .uncheck_start(package, entries.len());
        }
        let mut last_entries;
        for entry in entries {
            let Some(entry) = self.install_path.map_entry(entry) else {
                continue;
            };
            last_entries = self.commits.total_indexed();
            if let Err(e) = self
                .commits
                .remove_one(
                    Some(pkg),
                    &entry,
                    &self.install_path.base_dir,
                    self.skip_local_check,
                )
                .map_err(|e| Error::Pkgar(Box::new(e)))
            {
                if show_progress {
                    self.callback.borrow_mut().uncheck_end();
                }
                return Err(e);
            }
            if show_progress {
                self.callback
                    .borrow_mut()
                    .uncheck_increment(self.commits.total_indexed() - last_entries);
            }
        }
        // there's no I/O if skip_local_check enabled
        if show_progress {
            self.callback.borrow_mut().uncheck_end();
        }
        Ok(())
    }
}

impl Backend for PkgarBackend {
    fn install(&mut self, package: RemotePackage) -> Result<(), Error> {
        self.sync_keys()?;
        if package.package.version.is_empty() {
            return Ok(()); // metapackage
        }
        // TODO: Actually use that specific remote
        let (local_path, repo) = self
            .repo_manager
            .get_package_pkgar(&package.package.name, package.package.network_size)?;
        let mut pkg = PackageFile::new(&local_path, &repo.pubkey.unwrap())?;
        let entries = pkg.read_entries()?;
        self.install_inner(&package, &mut pkg, entries)?;

        self.create_head(&mut pkg, &package.package.name)?;
        Ok(())
    }

    fn uninstall(&mut self, package: PackageName) -> Result<(), Error> {
        if self.packages.protected.contains(&package) {
            return Err(Error::ProtectedPackage(package));
        }
        self.sync_keys()?;
        let mut pkg = self.get_package_head(&package)?;
        let entries = pkg.read_entries()?;
        self.uninstall_inner(&package, &pkg, entries)?;

        self.remove_package_head(&package)?;

        Ok(())
    }

    fn upgrade(&mut self, package: &RemotePackage) -> Result<(), Error> {
        self.sync_keys()?;

        let name = &package.package.name;
        let mut old = self.get_package_head(name)?;
        let (local_path, repo) = self
            .repo_manager
            .get_package_pkgar(name, package.package.network_size)?;
        let mut new = PackageFile::new(&local_path, &repo.pubkey.unwrap())?;
        let old_entries = old.read_entries()?;
        let new_entries = new.read_entries()?;
        let (to_install, to_update, to_remove) =
            self.commits.replace_diff(&old_entries, &new_entries)?;

        self.uninstall_inner(name, &old, to_remove)?;
        self.upgrade_inner(package, &mut new, to_update)?;
        self.install_inner(package, &mut new, to_install)?;
        self.create_head(&mut new, name)?;
        Ok(())
    }

    fn get_package_detail(&self, package: &PackageName) -> Result<RemotePackage, Error> {
        let (toml, remote) = self.repo_manager.get_package_toml(package)?;
        Ok(RemotePackage {
            package: Package::from_toml(&toml)?,
            remote,
        })
    }

    fn get_remote_detail(&self, package: &RemoteName) -> Result<RemotePath, Error> {
        self.repo_manager
            .get_remote_info(package)
            .map(|e| e.to_owned())
            .ok_or(Error::ValidRepoNotFound)
    }

    /// TODO: Multiple repository support
    fn get_repository_detail(&self) -> Result<Repository, Error> {
        let repo_str = PackageName::new("repo".to_string())?;
        let (toml, _) = self.repo_manager.get_package_toml(&repo_str)?;

        Ok(Repository::from_toml(&toml)?)
    }

    fn get_package_state(&self) -> PackageState {
        self.packages.clone()
    }

    fn commit_check(
        &self,
    ) -> Result<(&[pkgar::TransactionConflict], &[pkgar::TransactionIgnored]), Error> {
        Ok((
            self.commits.get_possible_conflicts(),
            self.commits.get_ignored_entries(),
        ))
    }

    fn commit_state(&mut self, new_state: PackageState) -> Result<usize, Error> {
        let mut transaction = core::mem::replace(&mut self.commits, Transaction::new());
        self.callback
            .borrow_mut()
            .commit_start(transaction.pending_commit());
        while transaction.pending_commit() > 0 {
            self.callback.borrow_mut().commit_increment(&transaction);
            if let Err(e) = transaction.commit_one() {
                return Err(Error::from(e));
            }
        }
        self.callback.borrow_mut().commit_end();

        self.packages = new_state;
        for (k, v) in &self.repo_manager.remote_map {
            let Some(pubkey) = v.pubkey else {
                return Err(Error::RepoNotLoaded(k.to_string()));
            };
            let pk = RepoPublicKeyFile::new(pubkey);
            self.packages.pubkeys.insert(k.to_string(), pk);
        }
        self.packages.to_sysroot(&self.install_path)?;
        Ok(transaction.total_committed())
    }

    fn abort_state(&mut self) -> Result<usize, Error> {
        let mut transaction = core::mem::replace(&mut self.commits, Transaction::new());
        self.callback
            .borrow_mut()
            .abort_start(transaction.pending_commit());
        while transaction.pending_commit() > 0 {
            self.callback.borrow_mut().commit_increment(&transaction);
            if let Err(e) = transaction.abort_one() {
                return Err(Error::from(e));
            }
        }
        self.callback.borrow_mut().abort_end();
        Ok(transaction.total_committed())
    }

    fn set_nocheck(&mut self, nocheck: bool) {
        self.skip_local_check = nocheck;
    }
}
