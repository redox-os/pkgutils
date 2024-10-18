use crate::PackageName;

#[derive(Default)]
pub struct PackageList {
    pub install: Vec<PackageName>,
    pub uninstall: Vec<PackageName>,
    //pub upgrade: Vec<PackageName>,
    //pub downgrade: Vec<PackageName>,
}
