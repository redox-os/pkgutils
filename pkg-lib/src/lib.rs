//#![cfg_attr(target_os = "redox", feature(io_error_more))]

#[cfg(feature = "library")]
pub mod backend;
#[cfg(feature = "library")]
pub mod callback;
#[cfg(feature = "library")]
pub use library::Library;
#[cfg(feature = "library")]
pub mod net_backend;
pub use package::{Package, PackageError, PackageInfo, PackageName, Repository, SourceIdentifier};
#[cfg(feature = "library")]
pub use package_state::{InstallState, PackageList, PackageState, RemoteName};
pub mod recipes;

#[cfg(feature = "library")]
mod library;
mod package;
#[cfg(feature = "library")]
mod package_state;
#[cfg(feature = "library")]
mod repo_manager;
#[cfg(feature = "library")]
mod sorensen;

#[cfg(feature = "library")]
const DOWNLOAD_DIR: &str = "/tmp/pkg_download/";
#[cfg(feature = "library")]
const PACKAGES_TOML_PATH: &str = "etc/pkg/packages.toml";
#[cfg(feature = "library")]
const PACKAGES_REMOTE_DIR: &str = "etc/pkg.d";
#[cfg(feature = "library")]
const PACKAGES_HEAD_DIR: &str = "var/lib/packages";
