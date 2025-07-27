use std::{
    borrow::Borrow,
    collections::{HashMap, VecDeque},
    env,
    ffi::{OsStr, OsString},
    fmt, fs,
    path::PathBuf,
};

use serde::de::{value::Error as DeError, Error as DeErrorT};
use serde_derive::{Deserialize, Serialize};
use toml::{self, from_str, to_string};

use crate::recipes::find;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Package {
    pub name: PackageName,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    pub target: String,
    //pub summary: String,
    //pub description: String,
    #[serde(default)]
    pub depends: Vec<PackageName>,
}

impl Package {
    pub fn new(name: &PackageName) -> Result<Self, PackageError> {
        let dir = find(name.as_str()).ok_or_else(|| PackageError::PackageNotFound(name.clone()))?;
        let target = env::var("TARGET").map_err(|_| PackageError::TargetInvalid)?;

        let file = dir.join("target").join(target).join("stage.toml");
        if !file.is_file() {
            return Err(PackageError::FileMissing(file));
        }

        let toml = fs::read_to_string(&file)
            .map_err(|err| PackageError::Parse(DeError::custom(err), Some(file.clone())))?;
        toml::from_str(&toml).map_err(|err| PackageError::Parse(DeError::custom(err), Some(file)))
    }

    pub fn new_recursive(
        names: &[PackageName],
        recursion: usize,
    ) -> Result<Vec<Self>, PackageError> {
        if recursion == 0 {
            return Err(PackageError::Recursion(Default::default()));
        }

        let mut packages = Vec::new();
        for name in names {
            let package = Self::new(name)?;

            let dependencies =
                Self::new_recursive(&package.depends, recursion - 1).map_err(|mut err| {
                    err.append_recursion(name);
                    err
                })?;

            for dependency in dependencies {
                if !packages.contains(&dependency) {
                    packages.push(dependency);
                }
            }

            if !packages.contains(&package) {
                packages.push(package);
            }
        }

        Ok(packages)
    }

    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }

    #[allow(dead_code)]
    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        // use error handling callbacks for this
        to_string(self).unwrap()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(into = "String")]
#[serde(try_from = "String")]
pub struct PackageName(String);

impl PackageName {
    pub fn new(name: impl Into<String>) -> Result<Self, PackageError> {
        let name = name.into();
        //TODO: are there any other characters that should be invalid?
        if name.is_empty() || name.contains(['.', '/', '\0']) {
            return Err(PackageError::PackageNameInvalid(name));
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
    type Error = PackageError;
    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::new(name)
    }
}

impl TryFrom<&str> for PackageName {
    type Error = PackageError;
    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::new(name)
    }
}

impl TryFrom<&OsStr> for PackageName {
    type Error = PackageError;
    fn try_from(name: &OsStr) -> Result<Self, Self::Error> {
        let name = name
            .to_str()
            .ok_or_else(|| PackageError::PackageNameInvalid(name.to_string_lossy().to_string()))?;
        Self::new(name)
    }
}

impl TryFrom<OsString> for PackageName {
    type Error = PackageError;
    fn try_from(name: OsString) -> Result<Self, Self::Error> {
        name.as_os_str().try_into()
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Borrow<str> for PackageName {
    fn borrow(&self) -> &str {
        self.as_str()
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

#[derive(Debug, serde::Deserialize)]
pub struct Repository {
    pub packages: HashMap<String, String>,
}

impl Repository {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }
}

/// Errors that occur while opening or parsing [`Package`]s.
///
/// These errors are unrecoverable but useful for reporting.
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("Missing package file {0:?}")]
    FileMissing(PathBuf),
    #[error("Package {0:?} name invalid")]
    PackageNameInvalid(String),
    #[error("Package {0:?} not found")]
    PackageNotFound(PackageName),
    #[error("Failed parsing package: {0}; file: {1:?}")]
    Parse(serde::de::value::Error, Option<PathBuf>),
    #[error("Recursion limit reached while processing dependencies; tree: {0:?}")]
    Recursion(VecDeque<PackageName>),
    #[error("TARGET triplet env var unset or invalid")]
    TargetInvalid,
}

impl PackageError {
    /// Append [`PackageName`] if the error is a recursion error.
    ///
    /// The [`PackageError::Recursion`] variant is a stack of package names that caused
    /// the recursion limit to be reached. This functions conditionally pushes a package
    /// name if the error is a recursion error to make it easier to build the stack.
    pub fn append_recursion(&mut self, name: &PackageName) {
        if let PackageError::Recursion(ref mut packages) = self {
            packages.push_front(name.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Package, PackageName};

    const WORKING_DEPENDS: &str = r#"
    name = "gzdoom"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = ["gtk3", "sdl2", "zmusic"]
    "#;

    const WORKING_NO_DEPENDS: &str = r#"
    name = "kmquake2"
    version = "TODO"
    target = "x86_64-unknown-redox"
    "#;

    const WORKING_EMPTY_DEPENDS: &str = r#"
    name = "iodoom3"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = []
    "#;

    const WORKING_EMPTY_VERSION: &str = r#"
    name = "dev-essentials"
    target = "x86_64-unknown-redox"
    depends = ["gcc13"]
    "#;

    const INVALID_NAME: &str = r#"
    name = "dolphin.emulator"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = ["qt5"]
    "#;

    const INVALID_NAME_DEPENDS: &str = r#"
    name = "mgba"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = ["ffmpeg.latest"]
    "#;

    #[test]
    fn deserialize_with_depends() -> Result<(), toml::de::Error> {
        let actual = Package::from_toml(WORKING_DEPENDS)?;
        let expected = Package {
            name: PackageName("gzdoom".into()),
            version: "TODO".into(),
            target: "x86_64-unknown-redox".into(),
            depends: vec![
                PackageName("gtk3".into()),
                PackageName("sdl2".into()),
                PackageName("zmusic".into()),
            ],
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_no_depends() -> Result<(), toml::de::Error> {
        let actual = Package::from_toml(WORKING_NO_DEPENDS)?;
        let expected = Package {
            name: PackageName("kmquake2".into()),
            version: "TODO".into(),
            target: "x86_64-unknown-redox".into(),
            depends: vec![],
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_empty_depends() -> Result<(), toml::de::Error> {
        let actual = Package::from_toml(WORKING_EMPTY_DEPENDS)?;
        let expected = Package {
            name: PackageName("iodoom3".into()),
            version: "TODO".into(),
            target: "x86_64-unknown-redox".into(),
            depends: vec![],
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_empty_version() -> Result<(), toml::de::Error> {
        let actual = Package::from_toml(WORKING_EMPTY_VERSION)?;
        let expected = Package {
            name: PackageName("dev-essentials".into()),
            version: "".into(),
            target: "x86_64-unknown-redox".into(),
            depends: vec![PackageName("gcc13".into())],
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    #[should_panic]
    fn deserialize_with_invalid_name_fails() {
        Package::from_toml(INVALID_NAME).unwrap();
    }

    #[test]
    #[should_panic]
    fn deserialize_with_invalid_dependency_name_fails() {
        Package::from_toml(INVALID_NAME_DEPENDS).unwrap();
    }

    #[test]
    fn roundtrip() -> Result<(), toml::de::Error> {
        let package = Package::from_toml(WORKING_DEPENDS)?;
        let package_roundtrip = Package::from_toml(&package.to_toml())?;

        assert_eq!(package, package_roundtrip);
        Ok(())
    }
}
