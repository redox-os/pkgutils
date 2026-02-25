use crate::{backend::Error, package::RemotePackage, PackageList, PackageName};

#[cfg(feature = "indicatif")]
pub use self::indicatif::IndicatifCallback;
pub use self::plain::PlainCallback;
#[cfg(feature = "indicatif")]
mod indicatif;
mod plain;

/// Implement callback to handle interaction
pub trait Callback {
    fn fetch_start(&mut self, initial_count: usize);
    fn fetch_package_name(&mut self, pkg_name: &PackageName);
    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize);
    fn fetch_end(&mut self);

    fn install_prompt(&mut self, list: &PackageList) -> Result<(), Error>;
    fn install_extract(&mut self, pkg_name: &RemotePackage);

    fn download_start(&mut self, length: u64, file: &str);
    fn download_increment(&mut self, downloaded: u64);
    fn download_end(&mut self);

    fn commit_start(&mut self, count: usize);
    fn commit_increment(&mut self, file: &pkgar::Transaction);
    fn commit_end(&mut self);

    fn abort_start(&mut self, count: usize);
    fn abort_increment(&mut self, file: &pkgar::Transaction);
    fn abort_end(&mut self);
}
