use crate::{backend::Error, callback::Callback, package::RemotePackage};

#[derive(Clone)]
pub struct SilentCallback {}

impl SilentCallback {
    pub fn new() -> Self {
        Self {}
    }
}

impl Callback for SilentCallback {
    fn fetch_start(&mut self, _: usize) {}

    fn fetch_package_name(&mut self, _: &crate::PackageName) {}

    fn fetch_package_increment(&mut self, _: usize, _: usize) {}

    fn fetch_end(&mut self) {}

    fn install_prompt(&mut self, _: &crate::PackageList) -> Result<(), Error> {
        Ok(())
    }

    fn install_extract(&mut self, _: &RemotePackage) {}

    fn download_start(&mut self, _: u64, _: &str) {}

    fn download_increment(&mut self, _: u64) {}

    fn download_end(&mut self) {}

    fn commit_start(&mut self, _: usize) {}

    fn commit_increment(&mut self, _: &pkgar::Transaction) {}

    fn commit_end(&mut self) {}

    fn abort_start(&mut self, _: usize) {}

    fn abort_increment(&mut self, _: &pkgar::Transaction) {}

    fn abort_end(&mut self) {}
}
