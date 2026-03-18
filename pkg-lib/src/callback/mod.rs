use crate::{package::RemotePackage, PackageName};

#[cfg(all(feature = "indicatif", feature = "library"))]
pub use self::indicatif::IndicatifCallback;
pub use self::plain::PlainCallback;
pub use self::silent::SilentCallback;
#[cfg(feature = "library")]
use crate::{backend::Error, PackageList};
#[cfg(all(feature = "indicatif", feature = "library"))]
mod indicatif;
mod plain;
mod silent;

/// Implement callback to handle interaction
pub trait Callback {
    fn fetch_start(&mut self, initial_count: usize);
    fn fetch_package_name(&mut self, pkg_name: &PackageName);
    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize);
    fn fetch_end(&mut self);

    #[cfg(feature = "library")]
    fn install_prompt(&mut self, list: &PackageList) -> Result<(), Error>;
    #[cfg(feature = "library")]
    fn install_check_conflict(
        &mut self,
        list: &Vec<pkgar::TransactionConflict>,
    ) -> Result<(), Error>;
    fn install_extract(&mut self, pkg_name: &RemotePackage);

    fn download_start(&mut self, length: u64, file: &str);
    fn download_increment(&mut self, downloaded: u64);
    fn download_end(&mut self);

    #[cfg(feature = "library")]
    fn commit_start(&mut self, count: usize);
    #[cfg(feature = "library")]
    fn commit_increment(&mut self, file: &pkgar::Transaction);
    #[cfg(feature = "library")]
    fn commit_end(&mut self);

    #[cfg(feature = "library")]
    fn abort_start(&mut self, count: usize);
    #[cfg(feature = "library")]
    fn abort_increment(&mut self, file: &pkgar::Transaction);
    #[cfg(feature = "library")]
    fn abort_end(&mut self);
}
