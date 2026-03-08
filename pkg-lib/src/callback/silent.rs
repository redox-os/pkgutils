use crate::{callback::Callback, package::RemotePackage};

#[cfg(feature = "library")]
use crate::backend::Error;

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

    #[cfg(feature = "library")]
    fn install_prompt(&mut self, _: &crate::PackageList) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(feature = "library")]
    fn install_check_conflict(&mut self, _: &Vec<pkgar::TransactionConflict>) -> Result<(), Error> {
        Ok(())
    }

    fn install_extract(&mut self, _: &RemotePackage) {}

    fn download_start(&mut self, _: u64, _: &str) {}

    fn download_increment(&mut self, _: u64) {}

    fn download_end(&mut self) {}

    #[cfg(feature = "library")]
    fn commit_start(&mut self, _: usize) {}

    #[cfg(feature = "library")]
    fn commit_increment(&mut self, _: &pkgar::Transaction) {}

    #[cfg(feature = "library")]
    fn commit_end(&mut self) {}

    #[cfg(feature = "library")]
    fn abort_start(&mut self, _: usize) {}

    #[cfg(feature = "library")]
    fn abort_increment(&mut self, _: &pkgar::Transaction) {}

    #[cfg(feature = "library")]
    fn abort_end(&mut self) {}
}
