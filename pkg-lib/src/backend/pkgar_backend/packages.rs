use crate::{package_list::PackageList, Package, PackageName};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// TODO: It's unclear what differentiate overall pkg library and pkgar backend since
// we're newly implemented installed list here, including public keys which is pkgar specific.

pub type RemoteName = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packages {
    pub protected: Vec<PackageName>,
    pub pubkeys: BTreeMap<RemoteName, PublicKeyFile>,
    pub installed: BTreeMap<PackageName, InstallState>,
}

#[derive(Serialize, Deserialize)]
pub struct InstallState {
    pub remote: RemoteName,
    pub blake3: String,
    pub manual: bool,
    // only matter during install
    #[serde(skip_serializing)]
    pub network_size: u64,
    pub storage_size: u64,
    pub dependencies: BTreeSet<PackageName>,
    pub dependents: BTreeSet<PackageName>,
}

impl Packages {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        toml::to_string(self).unwrap()
    }

    // mutably add valid packages to the graph.
    // Returns list of packages that need to be resolved,
    // which are not yet added to the package config.
    // If zero vector returned, it means all package deps are satisfied
    pub fn install(&mut self, packages: &[Package]) -> Vec<PackageName> {
        let rejected_packages = Vec::new();
        // for p in packages {
        //     p.
        // }

        rejected_packages
    }

    // mutably remove packages from the graph.
    // Returns list of packages that also need to be uninstalled,
    // which are not manually installed, so need be removed automatically.
    // If zero vector returned, it means all package deps are cleaned.
    pub fn uninstall(&mut self, packages: &[PackageName]) -> Vec<PackageName> {
        let obsolete_packages = Vec::new();
        // for p in packages {
        //     p.
        // }

        obsolete_packages
    }

    // Diff between old and new state, returns list of installed and uninstalled packages
    pub fn diff(&self, newer: &Self) -> PackageList {
        let diff = PackageList::default();

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
}

impl Default for Packages {
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
            ],
            pubkeys: Default::default(),
            installed: Default::default(),
        }
    }
}
