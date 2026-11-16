mod error;

use std::ffi::CStr;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::io::Write;
use std::ops::Deref;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
#[cfg(feature = "cli")]
use std::{path::PathBuf, sync::LazyLock};

use hex::FromHex;
use memsafe::Secret;
use pkgar_core::{
    PublicKey, SecretKey,
    dryoc::{
        classic::{
            crypto_pwhash::{PasswordHashAlgorithm, crypto_pwhash},
            crypto_secretbox::{Key, Nonce, crypto_secretbox_easy, crypto_secretbox_open_easy},
            crypto_sign::{crypto_sign_keypair, crypto_sign_seed_keypair},
        },
        constants::{CRYPTO_PWHASH_MEMLIMIT_INTERACTIVE, CRYPTO_PWHASH_OPSLIMIT_INTERACTIVE},
        types::NewByteArray,
    },
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "cli")]
use termion::input::TermRead;

type Salt = [u8; 32];

pub use error::Error;

#[cfg(feature = "cli")]
static HOMEDIR: LazyLock<PathBuf> = LazyLock::new(|| dirs::home_dir().unwrap_or("./".into()));

/// The default location for pkgar to look for the user's public key.
///
/// Defaults to `$HOME/.pkgar/keys/id_ed25519.pub.toml`. If `$HOME` is
/// unset, `./.pkgar/keys/id_ed25519.pub.toml`.
#[cfg(feature = "cli")]
pub static DEFAULT_PUBKEY: LazyLock<PathBuf> =
    LazyLock::new(|| Path::join(&HOMEDIR, ".pkgar/keys/id_ed25519.pub.toml"));

/// The default location for pkgar to look for the user's secret key.
///
/// Defaults to `$HOME/.pkgar/keys/id_ed25519.toml`. If `$HOME` is unset,
/// `./.pkgar/keys/id_ed25519.toml`.
#[cfg(feature = "cli")]
pub static DEFAULT_SECKEY: LazyLock<PathBuf> =
    LazyLock::new(|| Path::join(&HOMEDIR, ".pkgar/keys/id_ed25519.toml"));

mod ser {
    use hex::FromHex;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer};

    use crate::{Nonce, PublicKey, Salt};

    //TODO: Macro?
    pub(crate) fn to_salt<'d, D: Deserializer<'d>>(deser: D) -> Result<Salt, D::Error> {
        String::deserialize(deser)
            .and_then(|s| <[u8; 32]>::from_hex(s).map_err(|err| Error::custom(err.to_string())))
    }

    pub(crate) fn to_nonce<'d, D: Deserializer<'d>>(deser: D) -> Result<Nonce, D::Error> {
        String::deserialize(deser)
            .and_then(|s| <[u8; 24]>::from_hex(s).map_err(|err| Error::custom(err.to_string())))
    }

    pub(crate) fn to_pubkey<'d, D: Deserializer<'d>>(deser: D) -> Result<PublicKey, D::Error> {
        String::deserialize(deser)
            .and_then(|s| <[u8; 32]>::from_hex(s).map_err(|err| Error::custom(err.to_string())))
    }
}

/// Standard pkgar public key format definition. Use serde to serialize/deserialize
/// files into this struct (helper methods available).
#[derive(Clone, Deserialize, Serialize)]
pub struct PublicKeyFile {
    #[serde(serialize_with = "hex::serialize", deserialize_with = "ser::to_pubkey")]
    pub pkey: PublicKey,
}

impl PublicKeyFile {
    pub fn new(pubkey: PublicKey) -> Self {
        Self { pkey: pubkey }
    }

    /// Parse a `PublicKeyFile` from `file` (in toml format).
    pub fn open(file: impl AsRef<Path>) -> Result<Self, Error> {
        let content = fs::read_to_string(file).map_err(|source| Error::Io {
            source,
            path: None,
            context: "Reading public key",
        })?;
        Self::from_str(&content)
    }

    /// Get a public key from pkgar header
    pub fn from_header_raw(header: &pkgar_core::Header) -> Self {
        Self {
            pkey: header.public_key,
        }
    }

    /// Get a public key from toml content
    pub fn from_str(toml_str: &str) -> Result<Self, Error> {
        toml::from_str(&toml_str).map_err(Error::Deser)
    }

    /// Get a public key from path to pkgar header.
    /// The pkgar header will not be checked for validity.
    pub fn from_header_path(header: impl AsRef<Path>) -> Result<Self, Error> {
        let header = header.as_ref();
        let mut file = fs::File::open(header).map_err(|source| Error::Io {
            source,
            path: Some(header.into()),
            context: "Opening public key",
        })?;
        let mut buffer = vec![0u8; pkgar_core::HEADER_SIZE];
        file.read_exact(&mut buffer).map_err(|source| Error::Io {
            source,
            path: Some(header.into()),
            context: "Reading public key",
        })?;
        // SAFETY: We just want to read the public key, not to validate the header.
        let header = unsafe { pkgar_core::Header::new_unchecked(&buffer)? };
        Ok(Self::from_header_raw(header))
    }

    /// Write `self` serialized as toml to `w`.
    pub fn write(&self, mut w: impl Write) -> Result<(), Error> {
        w.write_all(toml::to_string(self)?.as_bytes())
            .map_err(|source| Error::Io {
                source,
                path: None,
                context: "Writing public key",
            })
    }

    /// Shortcut to write the public key to `file`
    pub fn save(&self, file: impl AsRef<Path>) -> Result<(), Error> {
        self.write(File::create(file).map_err(|source| Error::Io {
            source,
            path: None,
            context: "Writing public key",
        })?)
    }

    /// Get hex representation of this public key
    pub fn to_hex(&self) -> String {
        hex::encode(self.pkey)
    }
}

impl std::fmt::Debug for PublicKeyFile {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PublicKeyFile")
            .field("pkey", &hex::encode(self.pkey))
            .finish()
    }
}

enum SKey {
    Cipher([u8; 80]),
    Plain(SecretKey),
}

impl SKey {
    fn encrypt(&mut self, mut passwd: Passwd, salt: Salt, nonce: Nonce) -> Result<(), Error> {
        if let SKey::Plain(skey) = self {
            if let Some(passwd_key) = passwd.gen_key(salt) {
                let mut buf = [0; 80];
                crypto_secretbox_easy(&mut buf, skey.as_ref(), &nonce, &passwd_key)
                    .map_err(pkgar_core::Error::Dryoc)?;
                *self = SKey::Cipher(buf);
            }
        }
        Ok(())
    }

    fn decrypt(&mut self, mut passwd: Passwd, salt: Salt, nonce: Nonce) -> Result<(), Error> {
        if let SKey::Cipher(ciphertext) = self {
            let mut buf = [0; 64];
            if let Some(passwd_key) = passwd.gen_key(salt) {
                crypto_secretbox_open_easy(&mut buf, ciphertext.as_ref(), &nonce, &passwd_key)
                    .map_err(pkgar_core::Error::Dryoc)?;
            } else {
                let skey_plain = &ciphertext[..64];
                if skey_plain.len() != buf.len() {
                    return Err(Error::KeyInvalid {
                        expected: buf.len(),
                        actual: skey_plain.len(),
                    });
                }
                buf.copy_from_slice(skey_plain);
            }
            *self = SKey::Plain(buf);
        }
        Ok(())
    }
}

impl AsRef<[u8]> for SKey {
    fn as_ref(&self) -> &[u8] {
        match self {
            SKey::Cipher(buf) => buf.as_ref(),
            SKey::Plain(skey) => skey.as_ref(),
        }
    }
}

impl FromHex for SKey {
    type Error = hex::FromHexError;

    fn from_hex<T: AsRef<[u8]>>(buf: T) -> Result<SKey, hex::FromHexError> {
        let bytes = hex::decode(buf)?;

        // Public key is only 64 bytes...
        if bytes.len() == 64 {
            let mut buf = [0; 64];
            buf.copy_from_slice(&bytes);
            Ok(SKey::Plain(buf))
        } else {
            let mut buf = [0; 80];
            buf.copy_from_slice(&bytes);
            Ok(SKey::Cipher(buf))
        }
    }
}

/// Standard pkgar private key format definition. Use serde.
/// Internally, this struct stores the encrypted state of the private key as an enum.
/// Manipulate the state using the `encrypt()`, `decrypt()` and `is_encrypted()`.
#[derive(Deserialize, Serialize)]
pub struct SecretKeyFile {
    #[serde(serialize_with = "hex::serialize", deserialize_with = "ser::to_salt")]
    salt: Salt,
    #[serde(serialize_with = "hex::serialize", deserialize_with = "ser::to_nonce")]
    nonce: Nonce,
    #[serde(with = "hex")]
    skey: SKey,
}

impl SecretKeyFile {
    /// Generate a keypair with all the nessesary info to save both keys. You
    /// must call `save()` on each object to persist them to disk.
    pub fn new() -> (PublicKeyFile, SecretKeyFile) {
        let (pkey, skey) = crypto_sign_keypair();

        let pkey_file = PublicKeyFile { pkey };
        let skey_file = SecretKeyFile {
            salt: Salt::generate(),
            nonce: Nonce::generate(),
            skey: SKey::Plain(skey),
        };

        (pkey_file, skey_file)
    }

    /// Parse a `SecretKeyFile` from `file` (in toml format).
    pub fn open(file: impl AsRef<Path>) -> Result<Self, Error> {
        let content = fs::read_to_string(&file).map_err(|source| Error::Io {
            source,
            path: Some(file.as_ref().to_path_buf()),
            context: "Reading secret",
        })?;
        Self::from_str(&content)
    }

    /// Parse a `SecretKeyFile` from toml content
    pub fn from_str(toml_str: &str) -> Result<Self, Error> {
        toml::from_str(&toml_str).map_err(Error::Deser)
    }

    /// Write `self` serialized as toml to `w`.
    pub fn write(&self, mut w: impl Write) -> Result<(), Error> {
        w.write_all(toml::to_string(&self)?.as_bytes())
            .map_err(|source| Error::Io {
                source,
                path: None,
                context: "Writing secret",
            })?;
        Ok(())
    }

    /// Shortcut to write the secret key to `file`.
    ///
    /// Make sure to call `encrypt()` in order to encrypt
    /// the private key, otherwise it will be stored as plain text.
    pub fn save(&self, file: impl AsRef<Path>) -> Result<(), Error> {
        self.write(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&file)
                .map_err(|source| Error::Io {
                    source,
                    path: Some(file.as_ref().to_path_buf()),
                    context: "Opening file",
                })?,
        )
    }

    /// Ensure that the internal state of this struct is encrypted.
    /// Note that if passwd is empty, this function is a no-op.
    pub fn encrypt(&mut self, passwd: Passwd) -> Result<(), Error> {
        self.skey.encrypt(passwd, self.salt, self.nonce)
    }

    /// Ensure that the internal state of this struct is decrypted.
    /// If the internal state is already decrypted, this function is a no-op.
    pub fn decrypt(&mut self, passwd: Passwd) -> Result<(), Error> {
        self.skey.decrypt(passwd, self.salt, self.nonce)
    }

    /// Status of the internal state.
    pub fn is_encrypted(&self) -> bool {
        match self.skey {
            SKey::Cipher(_) => true,
            SKey::Plain(_) => false,
        }
    }

    /// Returns `None` if the secret key is encrypted.
    pub fn secret_key(&self) -> Option<SecretKey> {
        match &self.skey {
            SKey::Plain(skey) => Some(*skey),
            SKey::Cipher(_) => None,
        }
    }

    /// Returns `None` if the secret key is encrypted.
    pub fn public_key(&self) -> Option<PublicKey> {
        let skey = self.secret_key()?;
        let mut seed = [0; 32];
        seed.copy_from_slice(&skey[..32]);
        let (pkey, new_skey) = crypto_sign_seed_keypair(&seed);
        if skey != new_skey {
            // actually requires passphrase, but the user isn't provide one
            return None;
        }
        Some(pkey)
    }

    /// Returns `None` if the secret key is encrypted.
    pub fn public_key_file(&self) -> Option<PublicKeyFile> {
        Some(PublicKeyFile {
            pkey: self.public_key()?,
        })
    }
}

/// Secure in-memory representation of a password.
pub struct Passwd {
    bytes: Secret<4096>,
}

impl Passwd {
    /// Create a new `Passwd` and zero the old string.
    pub fn new(passwd: String) -> Result<Passwd, Error> {
        let pwd = Passwd {
            bytes: Secret::from_bytes(passwd.into_bytes())
                .map_err(|_| Error::PassphraseStoreError)?,
        };
        Ok(pwd)
    }

    /// Prompt the user for a `Passwd` on stdin.
    #[cfg(feature = "cli")]
    pub fn prompt(prompt: impl AsRef<str>) -> Result<Passwd, Error> {
        use std::io::{stdin, stdout};

        let stdout = stdout();
        let mut stdout = stdout.lock();
        let stdin = stdin();
        let mut stdin = stdin.lock();

        stdout
            .write_all(prompt.as_ref().as_bytes())
            .map_err(|source| Error::Io {
                source,
                path: None,
                context: "Writing prompt",
            })?;
        stdout.flush().map_err(|source| Error::Io {
            source,
            path: None,
            context: "Flushing prompt",
        })?;

        let Some(passwd) = stdin.read_passwd(&mut stdout).map_err(|source| Error::Io {
            source,
            path: None,
            context: "Reading passwd",
        })?
        else {
            return Err(Error::Io {
                source: std::io::Error::from(std::io::ErrorKind::UnexpectedEof),
                path: None,
                context: "Invalid Password Input",
            });
        };

        println!();

        Passwd::new(passwd)
    }

    /// Prompt for a password on stdin and confirm it. For configurable
    /// prompts, use [`Passwd::prompt`](struct.Passwd.html#method.prompt).
    #[cfg(feature = "cli")]
    pub fn prompt_new() -> Result<Passwd, Error> {
        let mut passwd = Passwd::prompt(
            "Please enter a new passphrase (leave empty to store the key in plaintext): ",
        )?;
        let mut confirm = Passwd::prompt("Please re-enter the passphrase: ")?;

        if !passwd.is_equal(&mut confirm) {
            return Err(Error::PassphraseMismatch);
        }
        Ok(passwd)
    }

    /// Get a key for symmetric key encryption from a password.
    pub(crate) fn gen_key(&mut self, salt: Salt) -> Option<Key> {
        let passwd = self.bytes.read().expect("Failed to access stored passwd");
        let passwd_cstr =
            CStr::from_bytes_until_nul(passwd.deref()).expect("Stored passwd is out of range");
        if !passwd_cstr.is_empty() {
            let mut key = [0; 32];
            crypto_pwhash(
                &mut key,
                passwd_cstr.to_bytes(),
                &salt,
                CRYPTO_PWHASH_OPSLIMIT_INTERACTIVE,
                CRYPTO_PWHASH_MEMLIMIT_INTERACTIVE,
                PasswordHashAlgorithm::Argon2id13,
            )
            .expect("Failed to get key from password");
            Some(key)
        } else {
            None
        }
    }

    pub fn is_equal(&mut self, other: &mut Self) -> bool {
        let (Ok(lhs), Ok(rhs)) = (self.bytes.read(), other.bytes.read()) else {
            return false;
        };
        lhs.deref() == rhs.deref()
    }
}

/// Generate a new keypair. The new keys will be saved to `file`. The user
/// will be prompted on stdin for a password, empty passwords will cause the
/// secret key to be stored in plain text. Note that parent
/// directories will not be created.
#[cfg(feature = "cli")]
pub fn gen_keypair(
    pkey_path: &Path,
    skey_path: &Path,
) -> Result<(PublicKeyFile, SecretKeyFile), Error> {
    let passwd = Passwd::prompt_new()?;

    let (pkey_file, mut skey_file) = SecretKeyFile::new();

    skey_file.encrypt(passwd)?;
    skey_file.save(skey_path)?;

    pkey_file.save(pkey_path)?;

    println!(
        "Generated {} and {}",
        pkey_path.display(),
        skey_path.display()
    );
    Ok((pkey_file, skey_file))
}

fn prompt_skey(
    skey_path: &Path,
    #[cfg(feature = "cli")] prompt: impl AsRef<str>,
) -> Result<SecretKeyFile, Error> {
    #[allow(unused_mut)]
    let mut key_file = SecretKeyFile::open(skey_path)?;

    #[cfg(feature = "cli")]
    if key_file.is_encrypted() {
        let passwd = Passwd::prompt(format!("{} {}: ", prompt.as_ref(), skey_path.display()))?;
        key_file.decrypt(passwd)?;
    }
    Ok(key_file)
}

/// Get a SecretKeyFile from a path. If the file is encrypted and "cli" feature enabled, prompt for a password on stdin.
/// Without "cli" feature the secret file will remain encypted as equivalent to calling `SecretKeyFile::open`.
pub fn get_skey(skey_path: &Path) -> Result<SecretKeyFile, Error> {
    prompt_skey(
        skey_path,
        #[cfg(feature = "cli")]
        "Passphrase for",
    )
}

/// Open, decrypt, re-encrypt with a different passphrase from stdin, and save the newly encrypted
/// secret key at `skey_path`.
#[cfg(feature = "cli")]
pub fn re_encrypt(skey_path: &Path) -> Result<(), Error> {
    let mut skey_file = prompt_skey(skey_path, "Old passphrase for")?;

    let passwd = Passwd::prompt_new()?;
    skey_file.encrypt(passwd)?;

    skey_file.save(skey_path)
}
