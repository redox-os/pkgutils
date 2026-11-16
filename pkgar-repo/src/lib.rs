pub use self::key::*;
pub use self::package::*;
pub use self::reader::*;
use std::error::Error as StdError;

mod cursor;
mod key;
mod package;
mod reader;

pub use ureq::Agent;

use std::fmt;
use std::time::Duration;

/// Whether a path is HTTP(S), which pkgar_repo can have support with
pub fn is_remote(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

/// Create a new HTTP client with good default configuration.
pub fn new_client() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(5)))
        .build();
    ureq::Agent::new_with_config(config)
}

#[derive(thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] pkgar_core::Error),
    #[error(transparent)]
    Key(#[from] pkgar_keys::Error),
    #[error(transparent)]
    Ureq(#[from] ureq::Error),
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{self}")?;

        let mut source = self.source();
        while let Some(err) = source {
            writeln!(f, "\tCaused by: {err}")?;
            source = err.source();
        }

        // if let Some(backtrace) = self.backtrace() {
        //     write!(f, "{backtrace:?}")?;
        // }

        Ok(())
    }
}
