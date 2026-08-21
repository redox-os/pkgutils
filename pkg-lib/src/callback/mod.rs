use crate::PackageName;

#[cfg(all(feature = "indicatif"))]
pub use self::indicatif::IndicatifCallback;
pub use self::plain::PlainCallback;
pub use self::silent::SilentCallback;
#[cfg(feature = "library")]
use crate::{backend::Error, package::RemotePackage, PackageList};
#[cfg(all(feature = "indicatif"))]
mod indicatif;
mod plain;
mod silent;

/// Implement callback to handle interaction. All interaction is divided by sections explained below.
/// Callback sections tipically have `begin` and  `end`, and `end` is always called once `begin` and
/// `end` with progress count being equal to total task can be considered a complete operation.
///
/// The order of which callback called are:
///
/// ## Fetch
///
/// Fetch callback is called at [`crate::Library::install`] and [`crate::Library::update`]
/// whenever new TOML file need to be downloaded. The total progress is measured by file,
/// while per-file download details is captured in Download callback.
///
/// ## Install Prompt
///
/// [`Self::install_prompt`] is called at [`crate::Library::apply`], right after *Fetch*.
/// The request can be denied by the callback. Then subsequent callback will be called.
//
/// ## Download
///
/// Download callback is called when downloading pkgar files. It is called per file,
///
/// This is also called during *Fetch* but `len` will be zero (unknown).
///
/// ## Extract
///
/// Extract callback is called when extracting pkgar files. It is called per file.
///
/// Pkgar files are immediately extracted after downloaded before moving to next file, so the
/// callback might jump from `Download` then `Extract` then repeat `Download` again.
///
/// ## Uncheck
///
/// Uncheck (Uninstall Check) callback is called when checking of removal for pkgar files. It is called per file.
///
/// This callback is only raised at [`crate::Library::uninstall`] without no-check flag. Otherwise,
/// it will not be called, because there's no I/O happening.
///
/// ## Install Check
///
/// [`Self::install_check`] is called at [`crate::Library::apply`], right before *Commit*.
/// The callback is raised when [`crate::Library::install`] or [`crate::Library::update`] has raised.
/// The request can be denied by the callback. Then *Commit* will be performed.
///
/// Uses [`pkgar::TransactionConflict::former_src`] to indicate that file of pkgar will be replaced by
/// [`pkgar::TransactionConflict::newer_src`]. `former_src` can also be [`None`] if it already installed,
/// which mean it will **NOT** being replaced to [`pkgar::TransactionConflict::newer_src`] unless with no-check flag.
///
/// ## Commit
///
/// Commit callback is called after extraction, the files will be placed to its destination
/// paths. It is called for overall transaction in  [`crate::Library::apply`].
/// [`Self::commit_increment`] always increment by one.
///
/// ## Abort
///
/// Abort callback is called after a failure happened before *Commit*, to undo *Extract* step.
/// (tipically because being canceled, network failure, file permission or running out of disk).
/// [`Self::abort_increment`] always increment by one.
///
pub trait Callback {
    fn fetch_start(&mut self, initial_count: usize);
    fn fetch_package_name(&mut self, pkg_name: &PackageName);
    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize);
    fn fetch_end(&mut self);

    #[cfg(feature = "library")]
    fn install_prompt(&mut self, list: &PackageList) -> Result<(), Error>;
    #[cfg(feature = "library")]
    fn install_check(
        &mut self,
        conflict: &[pkgar::TransactionConflict],
        ignored: &[pkgar::TransactionIgnored],
    ) -> Result<(), Error>;

    fn download_start(&mut self, length: u64, file: &str);
    fn download_increment(&mut self, downloaded: u64);
    fn download_end(&mut self);

    #[cfg(feature = "library")]
    fn extract_start(&mut self, pkg_name: &RemotePackage, index_count: usize);
    #[cfg(feature = "library")]
    fn extract_increment(&mut self, indexed: usize);
    #[cfg(feature = "library")]
    fn extract_end(&mut self);

    #[cfg(feature = "library")]
    fn commit_start(&mut self, count: usize);
    #[cfg(feature = "library")]
    fn commit_increment(&mut self, file: &pkgar::Transaction);
    #[cfg(feature = "library")]
    fn commit_end(&mut self);

    #[cfg(feature = "library")]
    fn uncheck_start(&mut self, pkg_name: &PackageName, index_count: usize);
    #[cfg(feature = "library")]
    fn uncheck_increment(&mut self, indexed: usize);
    #[cfg(feature = "library")]
    fn uncheck_end(&mut self);

    #[cfg(feature = "library")]
    fn abort_start(&mut self, count: usize);
    #[cfg(feature = "library")]
    fn abort_increment(&mut self, file: &pkgar::Transaction);
    #[cfg(feature = "library")]
    fn abort_end(&mut self);
}
