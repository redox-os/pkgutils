use crate::PackageName;

#[derive(Default)]
pub struct PackageList {
    pub install: Vec<PackageName>,
    pub uninstall: Vec<PackageName>,
    pub update: Vec<PackageName>,
    pub install_size: u64,
    pub network_size: u64,
    pub uninstall_size: u64,
}
