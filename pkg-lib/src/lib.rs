//#![cfg_attr(target_os = "redox", feature(io_error_more))]

#[cfg(feature = "library")]
pub mod backend;
#[cfg(feature = "library")]
pub use backend::Error;
#[cfg(feature = "library")]
pub mod callback;
#[cfg(feature = "library")]
pub use callback::Callback;
#[cfg(feature = "library")]
pub mod net_backend;
pub mod package;
pub use package::{Package, PackageInfo, PackageName};
pub mod recipes;

#[cfg(feature = "library")]
mod library;
#[cfg(feature = "library")]
mod package_list;
#[cfg(feature = "library")]
mod repo_manager;
#[cfg(feature = "library")]
mod sorensen;
#[cfg(feature = "library")]
pub use library::Library;

#[cfg(feature = "library")]
const DOWNLOAD_PATH: &str = "/tmp/pkg_download/";

// make them not relative
#[cfg(feature = "library")]
const PACKAGES_PATH: &str = "etc/pkg/packages.toml";
