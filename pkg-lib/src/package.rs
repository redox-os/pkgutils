use std::{borrow::Borrow, collections::HashMap, env, fmt, fs};

use serde_derive::{Deserialize, Serialize};
use toml::{self, from_str, to_string};

use crate::{recipes::find, Error};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Package {
    pub name: PackageName,
    pub version: String,
    pub target: String,
    //pub summary: String,
    //pub description: String,
    #[serde(default)]
    pub depends: Vec<PackageName>,
}

impl Package {
    pub fn new(name: &str) -> Result<Self, String> {
        let name: PackageName = name.try_into().map_err(|e| format!("{e:?}"))?;
        let dir = find(name.as_str());
        if dir.is_none() {
            return Err(format!("failed to find recipe directory '{}'", name));
        }
        let dir = dir.unwrap();
        let target =
            env::var("TARGET").map_err(|err| format!("failed to read TARGET: {:?}", err))?;

        let file = dir.join("target").join(target).join("stage.toml");
        if !file.is_file() {
            return Err(format!("failed to find package file '{}'", file.display()));
        }

        let toml = fs::read_to_string(&file).map_err(|err| {
            format!(
                "failed to read package file '{}': {}\n{:#?}",
                file.display(),
                err,
                err
            )
        })?;

        toml::from_str(&toml).map_err(|err| {
            format!(
                "failed to parse package file '{}': {}\n{:#?}",
                file.display(),
                err,
                err
            )
        })
    }

    pub fn new_recursive(names: &[&str], recursion: usize) -> Result<Vec<Self>, String> {
        if recursion == 0 {
            return Err(format!(
                "recursion limit while processing build dependencies: {:#?}",
                names
            ));
        }

        let mut packages = Vec::new();
        for name in names {
            let package = Self::new(name)?;

            // TODO: Ugly vec
            let dependencies: Vec<_> = package.depends.iter().map(Borrow::borrow).collect();
            let dependencies = Self::new_recursive(&dependencies, recursion - 1)
                .map_err(|err| format!("{}: failed on loading dependencies:\n{}", name, err))?;

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
    pub fn new(name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        //TODO: are there any other characters that should be invalid?
        if name.contains(['.', '/', '\0']) {
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

impl TryFrom<&str> for PackageName {
    type Error = Error;
    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Self::new(name)
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
