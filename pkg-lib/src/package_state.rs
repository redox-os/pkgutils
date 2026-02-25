use crate::{package::RemotePackage, PackageName};
use pkgar_keys::PublicKeyFile;
use serde_derive::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// Denotes that the string is a remote key
pub type RemoteName = String;

/// Contains current user packages state
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct PackageState {
    /// list of can't be accidentally uninstalled packages
    pub protected: BTreeSet<PackageName>,
    /// installed public keys per remote name.
    /// using pkgar_keys as a wrapper of dryoc public key.
    pub pubkeys: BTreeMap<RemoteName, PublicKeyFile>,
    /// install state per packages
    pub installed: BTreeMap<PackageName, InstallState>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct InstallState {
    pub remote: RemoteName,
    pub blake3: String,
    pub manual: bool,
    // only useful during install
    #[serde(skip_serializing)]
    pub network_size: u64,
    pub storage_size: u64,
    pub dependencies: BTreeSet<PackageName>,
    pub dependents: BTreeSet<PackageName>,
}

#[derive(Default, Debug, Clone)]
pub struct PackageList {
    pub install: Vec<PackageName>,
    pub uninstall: Vec<PackageName>,
    pub update: Vec<PackageName>,
    pub install_size: u64,
    pub network_size: u64,
    pub uninstall_size: u64,
}

impl PackageState {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        toml::to_string(self).unwrap()
    }

    // mutably add valid packages to the graph.
    /// Returns list of packages that need to be resolved,
    /// which are not yet added to the package config.
    /// If zero vector returned, it means all package deps are satisfied
    pub fn install(&mut self, packages: &[RemotePackage]) -> Vec<PackageName> {
        let mut missing_set = BTreeSet::new();
        let mut missing_deps = Vec::new();
        let package_names: BTreeSet<&PackageName> =
            packages.iter().map(|p| &p.package.name).collect();

        let mut recursion = 100;
        loop {
            let mut has_new_missing_deps = false;

            for pkg in packages {
                if missing_set.contains(&pkg.package.name) {
                    continue;
                }

                let mut has_missing_deps = false;
                for dep_name in &pkg.package.depends {
                    if self.installed.contains_key(dep_name) {
                    } else if !package_names.contains(dep_name) {
                        if missing_set.insert(dep_name.clone()) {
                            missing_deps.push(dep_name.clone());
                        }
                        has_missing_deps = true;
                    } else if missing_set.contains(dep_name) {
                        has_missing_deps = true;
                    } else {
                    }
                }

                if has_missing_deps {
                    if missing_set.insert(pkg.package.name.clone()) {
                        missing_deps.push(pkg.package.name.clone());
                    }
                    // dependents should be marked as missing well
                    has_new_missing_deps = true;
                }
            }

            if !has_new_missing_deps {
                break;
            }

            if recursion == 0 {
                panic!("Dependencies recursion exhausted");
            }
            recursion -= 1;
        }

        // all packages with their dependents should be satisfied
        let mut unsatisfied_deps: BTreeMap<PackageName, BTreeSet<PackageName>> = BTreeMap::new();
        for rpkg in packages {
            let pkg = &rpkg.package;
            if missing_set.contains(&pkg.name) {
                continue;
            }

            let (manual, dependents, remote) = if let Some(existing) = self.installed.get(&pkg.name)
            {
                (
                    existing.manual,
                    existing.dependents.clone(),
                    existing.remote.clone(),
                )
            } else {
                (
                    false,
                    unsatisfied_deps.remove(&pkg.name).unwrap_or_default(),
                    rpkg.remote.to_string(),
                )
            };

            let new_state = InstallState {
                remote,
                blake3: pkg.blake3.clone(),
                manual,
                network_size: pkg.network_size,
                storage_size: pkg.storage_size,
                dependencies: pkg.depends.iter().cloned().collect(),
                dependents,
            };

            self.installed.insert(pkg.name.clone(), new_state);

            for dep_name in &pkg.depends {
                if let Some(dep_state) = self.installed.get_mut(dep_name) {
                    dep_state.dependents.insert(pkg.name.clone());
                } else {
                    if let Some(dep_state) = unsatisfied_deps.get_mut(dep_name) {
                        dep_state.insert(pkg.name.clone());
                    } else {
                        let mut dep_state = BTreeSet::new();
                        dep_state.insert(pkg.name.clone());
                        unsatisfied_deps.insert(dep_name.clone(), dep_state);
                    }
                }
            }
        }

        if !unsatisfied_deps.is_empty() {
            panic!("Some unsatisfied deps are remained: {:?}", unsatisfied_deps);
        }

        missing_deps
    }

    // mutably remove packages from the graph.
    /// Returns list of packages that also need to be resolved,
    /// which are not all of their deps is listed in list of packages.
    /// If zero vector returned, it means uninstallation can be executed.
    pub fn uninstall(&mut self, packages: &[PackageName]) -> Vec<PackageName> {
        let mut pending_resolution = Vec::new();
        let mut packages_to_remove = packages.to_vec();

        // Filter out protected packages. Caller can wipe out the list beforehand to skip this behaviour.
        packages_to_remove.retain(|name| !self.protected.contains(name));

        let remove_set: BTreeSet<&PackageName> = packages_to_remove.iter().collect();
        let mut safe_to_remove = Vec::new();

        for name in &packages_to_remove {
            let Some(state) = self.installed.get(name) else {
                continue;
            };
            let missing_dependents: Vec<_> = state
                .dependents
                .iter()
                .cloned()
                .filter(|dep| !remove_set.contains(dep))
                .collect();
            let missing_dependencies: Vec<_> = state
                .dependencies
                .iter()
                .cloned()
                .filter(|dep| {
                    !remove_set.contains(dep) && !self.installed.get(dep).is_some_and(|p| p.manual)
                })
                .collect();

            if missing_dependents.is_empty() && missing_dependencies.is_empty() {
                safe_to_remove.push(name.clone());
            } else {
                pending_resolution.extend(missing_dependents);
                pending_resolution.push(name.clone());
                pending_resolution.extend(missing_dependencies);
            }
        }

        for name in safe_to_remove {
            if let Some(state) = self.installed.remove(&name) {
                for dep_name in &state.dependencies {
                    if let Some(dep_state) = self.installed.get_mut(dep_name) {
                        dep_state.dependents.remove(&name);
                    }
                }
            }
        }

        pending_resolution
    }

    // Diff between old and new state, returns list of installed and uninstalled packages
    pub fn diff(&self, newer: &Self) -> PackageList {
        let mut diff = PackageList::default();

        let mut old = self.installed.iter();
        let mut new = newer.installed.iter();
        let mut old_item = old.next();
        let mut new_item = new.next();

        loop {
            match (old_item, new_item) {
                (Some((k1, v1)), Some((k2, v2))) => match k1.cmp(k2) {
                    Ordering::Less => {
                        diff.uninstall.push(k1.clone());
                        diff.uninstall_size += v1.storage_size;
                        old_item = old.next();
                    }
                    Ordering::Greater => {
                        diff.install.push(k2.clone());
                        diff.install_size += v2.storage_size;
                        diff.network_size += v2.network_size;
                        new_item = new.next();
                    }
                    Ordering::Equal => {
                        if v1.blake3 != v2.blake3 {
                            diff.update.push(k1.clone());
                            diff.install_size += v2.storage_size;
                            diff.uninstall_size += v1.storage_size;
                            diff.network_size += v2.network_size;
                        }
                        old_item = old.next();
                        new_item = new.next();
                    }
                },
                (Some((k1, v1)), None) => {
                    diff.uninstall.push(k1.clone());
                    diff.uninstall_size += v1.storage_size;
                    old_item = old.next();
                }
                (None, Some((k2, v2))) => {
                    diff.install.push(k2.clone());
                    diff.install_size += v2.storage_size;
                    diff.network_size += v2.network_size;
                    new_item = new.next();
                }
                (None, None) => break,
            }
        }

        diff
    }

    pub fn get_installed_list(&self) -> Vec<PackageName> {
        self.installed.keys().cloned().collect()
    }

    /// Mark packages manually installed or not. Returns list of changed packages.
    /// PackageState are not marked automatically in any install mechanism.
    pub fn mark_as_manual(&mut self, manual: bool, packages: &[PackageName]) -> Vec<PackageName> {
        let mut marked = Vec::new();

        for package in packages {
            if let Some(pkg) = self.installed.get_mut(package) {
                if pkg.manual == manual {
                    continue;
                }
                pkg.manual = manual;
                marked.push(package.clone());
            }
        }
        marked
    }
}

impl Default for PackageState {
    fn default() -> Self {
        Self {
            // TODO: Hardcoded
            protected: vec![
                PackageName::new("kernel").unwrap(),
                PackageName::new("base-initfs").unwrap(),
                PackageName::new("base").unwrap(),
                PackageName::new("ion").unwrap(),
                PackageName::new("pkg").unwrap(),
                PackageName::new("relibc").unwrap(),
                PackageName::new("libgcc").unwrap(),
                PackageName::new("libstdcxx").unwrap(),
            ]
            .into_iter()
            .collect(),
            pubkeys: Default::default(),
            installed: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Package;

    use super::*;

    // --- Helper Functions for Test Data ---

    fn cpkg(name: &str) -> PackageName {
        PackageName::new(name).unwrap()
    }

    fn mock_package(name: &str, depends: Vec<&str>) -> RemotePackage {
        RemotePackage {
            package: Package {
                name: cpkg(name),
                version: "1.0.0".to_string(),
                target: "x86_64-unknown-redox".to_string(),
                blake3: "hash".to_string(),
                source_identifier: "src".to_string(),
                commit_identifier: "commit".to_string(),
                time_identifier: "time".to_string(),
                storage_size: 1000,
                network_size: 500,
                depends: depends.into_iter().map(|s| cpkg(s)).collect(),
            },
            remote: "origin".into(),
        }
    }

    fn mock_empty_db() -> PackageState {
        PackageState {
            protected: BTreeSet::new(),
            pubkeys: BTreeMap::new(),
            installed: BTreeMap::new(),
        }
    }

    #[test]
    fn test_install_simple_success() {
        let mut db = mock_empty_db();
        let nano = mock_package("nano", vec![]);
        let packages = vec![nano];
        let names = vec![cpkg("nano")];

        let missing = db.install(&packages);

        assert_eq!(missing, vec![]);
        assert_eq!(db.get_installed_list(), names);
        assert_eq!(db.installed[&cpkg("nano")].manual, false);
        assert_eq!(db.installed[&cpkg("nano")].remote, "origin");

        assert_eq!(db.mark_as_manual(true, &names), vec![cpkg("nano")]);
        assert_eq!(db.installed[&cpkg("nano")].manual, true);
    }

    #[test]
    fn test_install_missing_dependency() {
        let mut db = mock_empty_db();
        let bash = mock_package("bash", vec!["readline", "terminfo"]);
        let readline = mock_package("readline", vec!["ncurses"]);
        let ncurses = mock_package("ncurses", vec![]);
        let terminfo = mock_package("terminfo", vec![]);
        let packages = vec![bash, readline, terminfo, ncurses];
        // 1-st
        let missing = db.install(&packages[..1]);
        assert_eq!(
            missing,
            vec![cpkg("readline"), cpkg("terminfo"), cpkg("bash")]
        );
        assert_eq!(db.get_installed_list(), vec![]);
        // 2-nd
        let missing = db.install(&packages[..3]);
        assert_eq!(
            missing,
            vec![cpkg("ncurses"), cpkg("readline"), cpkg("bash")]
        );
        assert_eq!(db.get_installed_list(), vec![cpkg("terminfo")]);
        // 3-rd
        let missing = db.install(&packages[..]);
        assert_eq!(missing, vec![]);
        assert_eq!(
            db.get_installed_list(),
            vec![
                cpkg("bash"),
                cpkg("ncurses"),
                cpkg("readline"),
                cpkg("terminfo"),
            ]
        );

        assert_eq!(
            db.installed[&cpkg("bash")].dependents,
            vec![].iter().cloned().collect()
        );
        assert_eq!(
            db.installed[&cpkg("readline")].dependents,
            vec![cpkg("bash")].iter().cloned().collect()
        );
        assert_eq!(
            db.installed[&cpkg("ncurses")].dependents,
            vec![cpkg("readline")].iter().cloned().collect()
        );
    }

    #[test]
    fn test_uninstall_dependent() {
        let mut db = mock_empty_db();
        let base = mock_package("base", vec![]);
        let init = mock_package("base-initfs", vec!["redoxfs"]);
        let redoxfs = mock_package("redoxfs", vec![]);
        db.install(&[base, init, redoxfs]);
        let result = db.uninstall(&[cpkg("redoxfs")]);
        assert_eq!(
            db.get_installed_list(),
            vec![cpkg("base"), cpkg("base-initfs"), cpkg("redoxfs")]
        );
        assert_eq!(result, vec![cpkg("base-initfs"), cpkg("redoxfs")]);
        let result = db.uninstall(&result);
        assert_eq!(result, vec![]);
        assert_eq!(db.get_installed_list(), vec![cpkg("base")]);
    }

    #[test]
    fn test_uninstall_with_dependencies_unmarked() {
        let mut db = mock_empty_db();

        let gettext = mock_package("gettext", vec!["libiconv"]);
        let libiconv = mock_package("libiconv", vec![]);
        db.install(&[gettext, libiconv]);
        let result = db.uninstall(&[cpkg("gettext")]);
        assert_eq!(result, vec![cpkg("gettext"), cpkg("libiconv")]);
        assert_eq!(
            db.get_installed_list(),
            vec![cpkg("gettext"), cpkg("libiconv")]
        );
        let result = db.uninstall(&result);
        assert_eq!(result, vec![]);
        assert_eq!(db.get_installed_list(), vec![]);
    }

    #[test]
    fn test_uninstall_with_dependencies_marked() {
        let mut db = mock_empty_db();

        let gettext = mock_package("gettext", vec!["libiconv"]);
        let libiconv = mock_package("libiconv", vec![]);
        db.install(&[gettext, libiconv]);
        let result = db.mark_as_manual(true, &vec![cpkg("gettext"), cpkg("libiconv")]);
        assert_eq!(result.len(), 2usize);
        let result = db.uninstall(&[cpkg("gettext")]);
        assert_eq!(result, vec![]);
        assert_eq!(db.get_installed_list(), vec![cpkg("libiconv")]);
    }

    #[test]
    fn test_toml_integration() -> Result<(), toml::de::Error> {
        const TOML_DATA: &str = r#"
            [installed.bash]
            remote = "origin"
            blake3 = "abc"
            manual = true
            storage_size = 3000
            network_size = 2000
            dependencies = ["ncurses"]
            dependents = []

            [installed.ncurses]
            remote = "origin"
            blake3 = "def"
            manual = false
            storage_size = 2000
            network_size = 1000
            dependencies = []
            dependents = ["bash"]
        "#;

        let db: PackageState = PackageState::from_toml(TOML_DATA)?;

        assert_eq!(db.get_installed_list(), vec![cpkg("bash"), cpkg("ncurses")]);

        Ok(())
    }
}
