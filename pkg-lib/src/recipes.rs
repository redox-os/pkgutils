use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::PackageName;

static RECIPE_PATHS: LazyLock<HashMap<PackageName, PathBuf>> = LazyLock::new(|| {
    let mut recipe_paths = HashMap::new();
    for entry_res in ignore::Walk::new("recipes") {
        let Ok(entry) = entry_res else {
            continue;
        };
        if entry.file_name() == OsStr::new("recipe.toml") {
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
                eprintln!(
                    "recipe {:?} has two or more entries {:?}, skipping {:?}",
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
