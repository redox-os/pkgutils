use std::collections::BTreeMap;
use toml::{to_string, from_str};
use toml::de::Error as TomlDeError;
use std::io;

#[derive(Serialize, Deserialize)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub target: String,
    pub depends: Vec<String>,
}

#[derive(Debug,Fail)]
pub enum PackageMetaError {
    #[fail(display= "There was an error downloading your package(IO): $1")]
    IoError(io::Error),
    #[fail(display= "Toml Error: $1")]
    TomlError(TomlDeError),
}

impl From<io::Error> for PackageMetaError {
    fn from(err: io::Error) -> PackageMetaError {
        PackageMetaError::IoError(err)
    }
}

impl From<TomlDeError> for PackageMetaError {
    fn from(err: TomlDeError) -> PackageMetaError {
        PackageMetaError::TomlError(err)
    }
}

impl PackageMeta {
    pub fn new(name: &str, version: &str, target: &str, depends: Vec<String>) -> Self {
        PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            target: target.to_string(),
            depends: depends,
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, PackageMetaError> {
       Ok(from_str(text)?)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        to_string(self).unwrap()
    }
}

#[derive(Serialize, Deserialize)]
pub struct PackageMetaList {
    pub packages: BTreeMap<String, String>,
}

impl PackageMetaList {
    pub fn new() -> Self {
        PackageMetaList {
            packages: BTreeMap::new()
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, PackageMetaError> {
       Ok(from_str(text)?)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        to_string(self).unwrap()
    }
}
