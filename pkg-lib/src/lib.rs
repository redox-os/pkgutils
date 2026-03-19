pub mod backend;
pub mod callback;
#[cfg(feature = "library")]
pub use library::Library;
pub mod net_backend;
pub use package::*;
pub use package_state::*;
pub use repo_manager::*;
pub mod recipes;

#[cfg(feature = "library")]
mod library;
mod package;
mod package_state;
mod repo_manager;

#[cfg(feature = "library")]
mod sorensen;

const DOWNLOAD_DIR: &str = "/tmp/pkg_download/";
#[cfg(feature = "library")]
const PACKAGES_TOML_PATH: &str = "etc/pkg/packages.toml";
const PACKAGES_REMOTE_DIR: &str = "etc/pkg.d";
#[cfg(feature = "library")]
const PACKAGES_HEAD_DIR: &str = "var/lib/packages";
