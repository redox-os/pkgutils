use std::borrow::Borrow;
use std::cmp;
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{
    de::{value::Error as DeError, Error as ErrorT},
    Deserialize,
};

use crate::package::PackageError;
use crate::PackageName;

static RECIPE_PATHS: LazyLock<HashMap<PackageName, PathBuf>> = LazyLock::new(|| {
    let mut recipe_paths = HashMap::new();
    for entry_res in ignore::Walk::new("recipes") {
        let entry = entry_res.unwrap();
        if entry.file_name() == OsStr::new("recipe.sh")
            || entry.file_name() == OsStr::new("recipe.toml")
        {
            let recipe_file = entry.path();
            let Some(recipe_dir) = recipe_file.parent() else {
                continue;
            };
            let Some(recipe_name) = recipe_dir
                .file_name()
                .and_then(|x| x.to_str()?.try_into().ok())
            else {
                continue;
            };
            if let Some(other_dir) = recipe_paths.insert(recipe_name, recipe_dir.to_path_buf()) {
                panic!(
                    "recipe {:?} has two or more entries {:?}, {:?}",
                    recipe_dir.file_name(),
                    other_dir,
                    recipe_dir
                );
            }
        }
    }
    recipe_paths
});

pub fn find(recipe: &str) -> Option<&'static Path> {
    RECIPE_PATHS.get(recipe).map(PathBuf::as_path)
}

pub fn list(prefix: impl AsRef<Path>) -> BTreeSet<PathBuf> {
    let prefix = prefix.as_ref();
    RECIPE_PATHS
        .values()
        .map(|path| prefix.join(path))
        .collect()
}

/// Reverse dependencies of a package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReverseDependencies {
    pub recipe: PackageName,
    pub path: &'static Path,
    pub rev_deps: BTreeSet<ReverseDependency>,
}

impl ReverseDependencies {
    /// Retrieve the reverse dependencies of a recipe (packages that depend on this recipe).
    ///
    /// # Examples
    /// ```rust, no_run
    /// use pkg::{recipes::ReverseDependencies, package::PackageError};
    ///
    /// # fn main() -> Result<(), PackageError> {
    ///
    /// let query = ReverseDependencies::query("openssl1")?;
    /// assert!(query.rev_deps.contains("rustpython"));
    /// assert!(query.rev_deps.contains("servo"));
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn query(recipe: &str) -> Result<Self, PackageError> {
        let recipe = PackageName::new(recipe)?;
        let path =
            find(recipe.as_str()).ok_or_else(|| PackageError::PackageNotFound(recipe.clone()))?;

        let rev_deps = RECIPE_PATHS
            .iter()
            .filter_map(|(name, path)| {
                // While it's unlikely that a recipe would fail to parse, returning an error instead of
                // filtering out errors or unwrapping is cleaner.
                let package = match RecipeMinimal::from_toml(name.as_str()) {
                    Ok(package) => package,
                    // recipe.sh
                    Err(PackageError::FileMissing(_)) => return None,
                    Err(e) => return Some(Err(e)),
                };

                package
                    .build
                    .dependencies
                    .contains(&recipe)
                    .then_some(Ok(ReverseDependency {
                        name: name.clone(),
                        path,
                    }))
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            recipe,
            path,
            rev_deps,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReverseDependency {
    pub name: PackageName,
    pub path: &'static Path,
}

impl PartialOrd for ReverseDependency {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReverseDependency {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialEq for ReverseDependency {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl Eq for ReverseDependency {}

impl Borrow<str> for ReverseDependency {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

#[derive(Deserialize)]
struct RecipeMinimal {
    pub build: Build,
}

#[derive(Deserialize)]
struct Build {
    #[serde(default)]
    pub dependencies: Vec<PackageName>,
}

impl RecipeMinimal {
    fn from_toml(recipe: impl AsRef<str>) -> Result<Self, PackageError> {
        let name = PackageName::new(recipe.as_ref())?;
        let recipe = find(recipe.as_ref()).ok_or(PackageError::PackageNotFound(name))?;
        let file = recipe.join("recipe.toml");
        if !file.is_file() {
            return Err(PackageError::FileMissing(file));
        }

        fs::read(&file)
            .map_err(|err| PackageError::Parse(DeError::custom(err), Some(file.clone())))
            .and_then(|v| {
                String::from_utf8(v)
                    .map_err(|err| PackageError::Parse(DeError::custom(err), Some(file.clone())))
            })
            .and_then(|data| {
                toml::de::from_str(&data)
                    .map_err(|err| PackageError::Parse(DeError::custom(err), Some(file.clone())))
            })
    }
}
