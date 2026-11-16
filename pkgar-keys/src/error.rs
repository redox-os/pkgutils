use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] pkgar_core::Error),
    #[error("{source} ({path:?}) {context:?}")]
    Io {
        #[source]
        source: io::Error,
        path: Option<PathBuf>,
        context: &'static str,
    },
    #[error(transparent)]
    Ser(#[from] toml::ser::Error),
    #[error(transparent)]
    Deser(#[from] toml::de::Error),
    #[error("Invalid cypher key length (expected {expected}, got {actual})")]
    KeyInvalid { expected: usize, actual: usize },
    #[error("KeyMismatch")]
    KeyMismatch,
    #[error("Invalid nonce length")]
    NonceInvalid,
    #[error("Incorrect passphrase")]
    PassphraseIncorrect,
    #[error("Passphrases did not match")]
    PassphraseMismatch,
    #[error("A passphrase to decrypt secret key is required")]
    PassphraseRequired,
    #[error("Unable to access or store passphrase, or the password is too long")]
    PassphraseStoreError,
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
