pub mod backend;
pub mod callback;
#[cfg(feature = "library")]
pub use library::Library;
pub mod net_backend;
pub use package::{Package, PackageError, PackageInfo, PackageName, Repository, SourceIdentifier};
#[cfg(feature = "library")]
pub use package_state::{InstallState, PackageList, PackageState};
pub mod recipes;

#[cfg(feature = "library")]
mod library;
mod package;
#[cfg(feature = "library")]
mod package_state;
pub mod repo_manager;

#[cfg(feature = "library")]
mod sorensen;

const DOWNLOAD_DIR: &str = "/tmp/pkg_download/";
#[cfg(feature = "library")]
const PACKAGES_TOML_PATH: &str = "etc/pkg/packages.toml";
const PACKAGES_REMOTE_DIR: &str = "etc/pkg.d";
#[cfg(feature = "library")]
const PACKAGES_HEAD_DIR: &str = "var/lib/packages";
