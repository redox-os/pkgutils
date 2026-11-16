mod bin;
pub mod ext;
mod package;
#[cfg(feature = "repo")]
pub(crate) mod repo;
mod transaction;

pub use bin::*;
pub use package::*;
#[cfg(feature = "repo")]
pub use repo::*;
pub use transaction::*;

use std::io;
use std::path::PathBuf;

use pkgar_core::Entry;

const READ_WRITE_HASH_BUF_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] pkgar_core::Error),
    #[error(transparent)]
    Keys(#[from] pkgar_keys::Error),
    #[error(transparent)]
    #[cfg(feature = "repo")]
    Repo(#[from] pkgar_repo::Error),
    #[error("{source} ({path:?}) {context:?}")]
    Io {
        #[source]
        source: io::Error,
        path: Option<PathBuf>,
        context: &'static str,
    },
    #[error(
        "Failed to commit transaction ({changed} files changed, {remaining} files remaining): {source}"
    )]
    FailedCommit {
        #[source]
        source: Box<Self>,
        changed: usize,
        remaining: usize,
    },
    #[error("Invalid component '{}' in path '{}'{}", invalid.display(), path.display(), entry.as_ref().map(|_| " while parsing entry").unwrap_or_default())]
    InvalidPathComponent {
        invalid: PathBuf,
        path: PathBuf,
        entry: Option<Box<Entry>>,
    },
    #[error("Entry size mismatch: expected {expected}; got {actual}")]
    LengthMismatch { actual: u64, expected: u64 },
    #[error("Data not initialized.")]
    DataNotInitialized,
}

macro_rules! wrap_io_err {
    ($context:expr) => {
        |source| Error::Io {
            source,
            path: None,
            context: $context,
        }
    };
    ($path:expr, $context:expr) => {
        |source| Error::Io {
            source,
            path: Some($path.to_path_buf()),
            context: $context,
        }
    };
}

pub(crate) use wrap_io_err;
