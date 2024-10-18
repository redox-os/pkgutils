use serde_derive::{Deserialize, Serialize};
use std::fmt;
use toml::{self, from_str, to_string};

use crate::Error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Package {
    pub name: PackageName,
    pub version: String,
    pub target: String,
    //pub summary: String,
    //pub description: String,
    pub depends: Vec<PackageName>,
}

/*impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}*/

impl Package {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }

    #[allow(dead_code)]
    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        // use error handeling callbacks for this
        to_string(self).unwrap()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(into = "String")]
#[serde(try_from = "String")]
pub struct PackageName(String);

impl PackageName {
    pub fn new(name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        //TODO: are there any other characters that should be invalid?
        if name.contains('.') || name.contains('/') || name.contains('\0') {
            return Err(Error::PackageNameInvalid(name));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<PackageName> for String {
    fn from(package_name: PackageName) -> Self {
        package_name.0
    }
}

impl TryFrom<String> for PackageName {
    type Error = Error;
    fn try_from(name: String) -> Result<Self, Error> {
        Self::new(name)
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct PackageInfo {
    pub installed: bool,
    pub version: String,
    pub target: String,

    pub download_size: String,
    // pub install_size: String,
    pub depends: Vec<PackageName>,
}
