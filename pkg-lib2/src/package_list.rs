#[derive(Default)]
pub struct PackageList {
    pub install: Vec<String>,
    pub uninstall: Vec<String>,
    //pub upgrade: Vec<String>,
    //pub downgrade: Vec<String>,
}
