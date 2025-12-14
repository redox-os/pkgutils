use std::{
    borrow::Borrow,
    collections::{BTreeMap, VecDeque},
    env,
    ffi::{OsStr, OsString},
    fmt, fs,
    path::PathBuf,
};

use serde::de::{value::Error as DeError, Error as DeErrorT};
use serde_derive::{Deserialize, Serialize};
use toml::{self, from_str, to_string};

use crate::recipes::find;

fn is_zero(n: &u64) -> bool {
    *n == 0
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, PartialOrd)]
#[serde(default)]
pub struct Package {
    /// package name
    pub name: PackageName,
    /// package version
    #[serde(skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// platform target
    pub target: String,
    /// hash in pkgar head
    #[serde(skip_serializing_if = "String::is_empty")]
    pub blake3: String,
    /// git commit or tar hash of source package
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_identifier: String,
    /// git commit of redox repository
    #[serde(skip_serializing_if = "String::is_empty")]
    pub commit_identifier: String,
    /// time when this package published in IS0 8601
    #[serde(skip_serializing_if = "String::is_empty")]
    pub time_identifier: String,
    /// size of files (uncompressed)
    #[serde(skip_serializing_if = "is_zero")]
    pub storage_size: u64,
    /// size of pkgar (maybe compressed)
    #[serde(skip_serializing_if = "is_zero")]
    pub network_size: u64,
    /// dependencies
    pub depends: Vec<PackageName>,
}

impl Package {
    pub fn new(name: &PackageName) -> Result<Self, PackageError> {
        let dir = find(name.name()).ok_or_else(|| PackageError::PackageNotFound(name.clone()))?;
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
        nonstop: bool,
        recursion: usize,
    ) -> Result<Vec<Self>, PackageError> {
        if names.len() == 0 {
            return Ok(vec![]);
        }
        let (list, map) = Self::new_recursive_nonstop(names, recursion);
        if nonstop && list.len() > 0 {
            Ok(list)
        } else if !nonstop && map.len() == list.len() {
            Ok(list)
        } else {
            let (_, res) = map.into_iter().find(|(_, v)| v.is_err()).unwrap();
            Err(res.err().unwrap())
        }
    }

    // list ordered success packages and map of failed packages
    // a package can be both success and failed if dependencies aren't satistied
    pub fn new_recursive_nonstop(
        names: &[PackageName],
        recursion: usize,
    ) -> (Vec<Self>, BTreeMap<PackageName, Result<(), PackageError>>) {
        let mut packages = Vec::new();
        let mut packages_map = BTreeMap::new();
        for name in names {
            if packages_map.contains_key(name) {
                continue;
            }

            let package = if recursion == 0 {
                Err(PackageError::Recursion(Default::default()))
            } else {
                Self::new(name)
            };

            match package {
                Ok(package) => {
                    let mut has_invalid_dependency = false;
                    let (dependencies, dependencies_map) =
                        Self::new_recursive_nonstop(&package.depends, recursion - 1);
                    for dependency in dependencies {
                        if !packages_map.contains_key(&dependency.name) {
                            packages_map.insert(dependency.name.clone(), Ok(()));
                            packages.push(dependency);
                        }
                    }
                    for (dep_name, result) in dependencies_map {
                        if let Err(mut e) = result {
                            if !packages_map.contains_key(&dep_name) {
                                e.append_recursion(name);
                                packages_map.insert(dep_name, Err(e));
                            }
                            has_invalid_dependency = true;
                        }
                    }
                    // TODO: this if check is redundant
                    if !packages_map.contains_key(name) {
                        packages_map.insert(
                            name.clone(),
                            if has_invalid_dependency {
                                Err(PackageError::DependencyInvalid(name.clone()))
                            } else {
                                Ok(())
                            },
                        );
                        packages.push(package);
                    }
                }
                Err(e) => {
                    packages_map.insert(name.clone(), Err(e));
                }
            }
        }

        (packages, packages_map)
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

/// A package name is valid in these formats:
///
/// + `recipe` A recipe on mandatory package
/// + `recipe.pkg` A recipe on "pkg" optional package
/// + `host:recipe` A recipe with host target on mandatory package
/// + `host:recipe.pkg` A recipe with host target on "pkg" optional package
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(into = "String")]
#[serde(try_from = "String")]
pub struct PackageName(String);

impl PackageName {
    pub fn new(name: impl Into<String>) -> Result<Self, PackageError> {
        let name = name.into();
        //TODO: are there any other characters that should be invalid?
        if name.is_empty() {
            return Err(PackageError::PackageNameInvalid(name));
        }
        let mut separators = 0;
        let mut has_host_prefix = false;
        for c in name.chars() {
            if "/\0".contains(c) {
                return Err(PackageError::PackageNameInvalid(name));
            }
            if c == '.' {
                separators += 1;
                if separators > 1 {
                    return Err(PackageError::PackageNameInvalid(name));
                }
            }
            if c == ':' {
                if has_host_prefix {
                    return Err(PackageError::PackageNameInvalid(name));
                }
                has_host_prefix = true;
            }
        }
        let r = Self(name);
        if has_host_prefix && !r.is_host() {
            return Err(PackageError::PackageNameInvalid(r.0));
        }
        Ok(r)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check if "host:" prefix exists
    pub fn is_host(&self) -> bool {
        self.0.starts_with("host:")
    }

    /// Get the name between "host:" prefix and ".pkg" suffix
    pub fn name(&self) -> &str {
        let mut s = self.0.as_str();
        if self.is_host() {
            s = &s[5..]
        }
        if let Some(pos) = s.find('.') {
            s = &s[..pos]
        }
        s
    }

    /// Get ".pkg" suffix
    pub fn suffix(&self) -> Option<&str> {
        let mut s = self.0.as_str();
        if self.is_host() {
            s = &s[5..]
        }
        if let Some(pos) = s.find('.') {
            Some(&s[pos + 1..])
        } else {
            None
        }
    }

    /// Strip "host:" prefix if exists
    pub fn without_host(&self) -> PackageName {
        let name = if self.is_host() {
            &self.as_str()["host:".len()..]
        } else {
            self.as_str()
        };

        Self(name.to_string())
    }

    /// Add "host:" prefix if not exists
    pub fn with_host(&self) -> PackageName {
        let name = if self.is_host() {
            self.as_str().to_string()
        } else {
            format!("host:{}", self.as_str())
        };

        Self(name)
    }

    /// Add or replace suffix. Does not retain "host:" prefix
    pub fn with_suffix(&self, suffix: Option<&str>) -> PackageName {
        let mut name = self.name().to_string();
        if let Some(suffix) = suffix {
            name.push('.');
            name.push_str(suffix);
        }

        Self(name)
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
    pub package: Package,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceIdentifier {
    /// git commit or tar hash
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_identifier: String,
    /// git commit of redox repository
    #[serde(skip_serializing_if = "String::is_empty")]
    pub commit_identifier: String,
    /// time when source updated in IS0 8601
    #[serde(skip_serializing_if = "String::is_empty")]
    pub time_identifier: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Repository {
    /// list of published packages
    pub packages: BTreeMap<String, String>,
    /// list of outdated/missing packages, with source identifier when it first time went outdated/missing
    pub outdated_packages: BTreeMap<String, SourceIdentifier>,
}

impl Repository {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }
}

/// Errors that occur while opening or parsing [`Package`]s.
///
/// These errors are unrecoverable but useful for reporting.
#[derive(Clone, Debug, thiserror::Error)]
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
    #[error("Package {0:?} is missing one or more dependencies")]
    DependencyInvalid(PackageName),
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
    use std::collections::BTreeMap;

    use crate::package::{Repository, SourceIdentifier};

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

    const WORKING_REPOSITORY: &str = r#"
    [packages]
    foo = "bar"
    "#;

    const WORKING_OUTDATED_REPOSITORY: &str = r#"
    [outdated_packages.gnu-make]
    source_identifier = "1a0e5353205e106bd9b3c0f4a5f37ee1156a1e1c8feb771d1b4842c216612cba"
    commit_identifier = "da93b635fec96a6fac7da9bf7742d850cbce68b4"
    time_identifier = "2025-12-13T05:33:07Z"
    "#;

    const INVALID_NAME: &str = r#"
    name = "dolphin.emu.lator"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = ["qt5"]
    "#;

    const INVALID_NAME_DEPENDS: &str = r#"
    name = "mgba"
    version = "TODO"
    target = "x86_64-unknown-redox"
    depends = ["ffmpeg:latest"]
    "#;

    #[test]
    fn package_name_split() -> Result<(), toml::de::Error> {
        let name1 = PackageName::new("foo").unwrap();
        let name2 = PackageName::new("foo.bar").unwrap();
        let name3 = PackageName::new("host:foo").unwrap();
        let name4 = PackageName::new("host:foo.").unwrap();
        assert_eq!(
            (name1.name(), name1.is_host(), name1.suffix()),
            ("foo", false, None)
        );
        assert_eq!(
            (name2.name(), name2.is_host(), name2.suffix()),
            ("foo", false, Some("bar"))
        );
        assert_eq!(
            (name3.name(), name3.is_host(), name3.suffix()),
            ("foo", true, None)
        );
        assert_eq!(
            (name4.name(), name4.is_host(), name4.suffix()),
            ("foo", true, Some(""))
        );
        Ok(())
    }

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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_empty_version() -> Result<(), toml::de::Error> {
        let actual = Package::from_toml(WORKING_EMPTY_VERSION)?;
        let expected = Package {
            name: PackageName("dev-essentials".into()),
            target: "x86_64-unknown-redox".into(),
            depends: vec![PackageName("gcc13".into())],
            ..Default::default()
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_repository() -> Result<(), toml::de::Error> {
        let actual = Repository::from_toml(WORKING_REPOSITORY)?;
        let expected = Repository {
            packages: BTreeMap::from([("foo".into(), "bar".into())]),
            ..Default::default()
        };

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn deserialize_repository_outdated() -> Result<(), toml::de::Error> {
        let actual = Repository::from_toml(WORKING_OUTDATED_REPOSITORY)?;
        let expected = Repository {
            outdated_packages: BTreeMap::from([(
                "gnu-make".into(),
                SourceIdentifier {
                    source_identifier:
                        "1a0e5353205e106bd9b3c0f4a5f37ee1156a1e1c8feb771d1b4842c216612cba".into(),
                    commit_identifier: "da93b635fec96a6fac7da9bf7742d850cbce68b4".into(),
                    time_identifier: "2025-12-13T05:33:07Z".into(),
                },
            )]),
            ..Default::default()
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
