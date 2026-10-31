#[cfg(feature = "library")]
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

use std::path::{Path, PathBuf};

/// Path where it determines where to install and other operations
#[derive(Clone, Debug)]
pub struct PathIndex {
    /// Absolute path to install
    pub base_dir: PathBuf,
    /// Relative path to config dir: "etc" or "$XDG_CONFIG_HOME/"
    pub config_dir: PathBuf,
    /// Relative path to usr dir: "usr" or "~/.local/"
    pub usr_dir: PathBuf,
    /// Relative path to data dir: "share" or "~/.local/share/" ($XDG_DATA_DIR)
    pub share_dir: PathBuf,
    /// Absolute path to download: /tmp/pkg_download or $XDG_RUNTIME_DIR/pkg_download
    pub download_dir: PathBuf,
    /// Absolute path to config repos: /etc/pkg.d or $XDG_CONFIG_HOME/pkg.d
    pub config_repos: PathBuf,
    /// Absolute path to config files: /etc/pkg or $XDG_CONFIG_HOME/pkg
    pub config_files: PathBuf,
    /// Absolute path to package heads: /var/lib/packages or "~/.local/state/packages" ($XDG_STATE_HOME)
    pub var_heads: PathBuf,
    /// Fast check if any of *_dir variables are mapped
    pub use_xdg: bool,
}

impl PathIndex {
    pub fn from_base_path(base_dir: PathBuf) -> PathIndex {
        #[cfg(feature = "library")]
        let (home, use_xdg) = {
            let home = dirs::home_dir();
            let use_xdg = home.as_ref().is_some_and(|s| *s == base_dir);
            (home, use_xdg)
        };
        #[cfg(not(feature = "library"))]
        let (home, use_xdg) = (None::<PathBuf>, false);
        Self::from_base_path_and_xdg(base_dir, home.as_deref(), use_xdg)
    }
    pub fn from_base_path_and_xdg(
        base_dir: PathBuf,
        home: Option<&Path>,
        use_xdg: bool,
    ) -> PathIndex {
        let (config, state, data, runtime, use_xdg) = paths(use_xdg, home.clone());
        let download_dir = match &runtime {
            Some(r) => r.join("pkg_download"),
            None => PathBuf::from("/tmp/pkg_download"),
        };
        let config_dir = match config {
            Some(r) => r,
            None if use_xdg => PathBuf::from(".config"),
            None => PathBuf::from("etc"),
        };
        let share_dir = match data {
            Some(r) => r,
            None if use_xdg => PathBuf::from(".local/share"),
            None => PathBuf::from("usr/share"),
        };
        let state_dir = match state {
            Some(r) => r,
            None if use_xdg => PathBuf::from(".local/state"),
            None => PathBuf::from("var/lib"),
        };
        let usr_dir = match use_xdg {
            true => PathBuf::from(".local"),
            false => PathBuf::from("usr"),
        };
        let config_repos = base_dir.join(&config_dir).join("pkg.d");
        let config_files = base_dir.join(&config_dir).join("pkg");
        let var_heads = base_dir.join(&state_dir).join("packages");

        PathIndex {
            base_dir,
            config_dir,
            usr_dir,
            share_dir,
            download_dir,
            config_repos,
            config_files,
            var_heads,
            use_xdg,
        }
    }

    pub fn packages_toml_path(&self) -> PathBuf {
        self.config_files.join("packages.toml")
    }
    #[cfg(feature = "library")]
    pub fn current_home_dir() -> PathBuf {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    }
    #[cfg(feature = "library")]
    pub fn map_entry(&self, mut entry: pkgar_core::Entry) -> Option<pkgar_core::Entry> {
        if entry.path.starts_with(b".pkg/") {
            return None; // internal pkg items (e.g. postinstall)
        } else if !self.use_xdg {
            return Some(entry);
        }

        let mut mapping = None;
        let paths: [(&[u8], &PathBuf); _] = [
            (b"etc/", &self.config_dir),
            (b"usr/share/", &self.share_dir),
            (b"usr/bin/", &self.usr_dir.join("bin")),
            (b"usr/games/", &self.usr_dir.join("games")),
            (b"usr/include/", &self.usr_dir.join("include")),
            (b"usr/lib/", &self.usr_dir.join("lib")),
            (b"usr/libexec/", &self.usr_dir.join("libexec")),
        ];
        for (path, dir) in paths {
            if entry.path.starts_with(path) {
                if self.config_dir == Path::new(dir) {
                    return Some(entry); // identical
                }
                mapping = Some((path.len(), dir.clone()));
            }
        }

        let Some((prefix_len, mut base_dir)) = mapping else {
            // invalid entry, TODO: error out
            println!(
                "skipping entry due to invalid location: {}",
                OsStr::from_bytes(entry.path_bytes()).display()
            );
            return None;
        };

        let path_slice = entry.path_bytes();
        let remainder = &path_slice[prefix_len..];

        base_dir.push(OsStr::from_bytes(remainder));

        let new_path_bytes = base_dir.as_os_str().as_bytes();

        if new_path_bytes.len() > 256 {
            // TODO: properly handle this?
            return None;
        }

        let mut new_array = [0u8; 256];
        new_array[..new_path_bytes.len()].copy_from_slice(new_path_bytes);

        entry.path = new_array;
        Some(entry)
    }
}

#[cfg_attr(not(feature = "library"), expect(unused))]
fn paths(
    use_xdg: bool,
    home: Option<&Path>,
) -> (
    Option<PathBuf>,
    Option<PathBuf>,
    Option<PathBuf>,
    Option<PathBuf>,
    bool,
) {
    #[cfg(feature = "library")]
    if use_xdg {
        let make_relative = |path: Option<PathBuf>| -> Option<PathBuf> {
            path.and_then(|p| home.and_then(|h| p.strip_prefix(h).map(|s| s.to_path_buf()).ok()))
        };
        // based on dirs code, all except runtime dir is guaranteed to present if home is available.
        let config = make_relative(dirs::config_dir());
        let state = make_relative(dirs::state_dir());
        let data = make_relative(dirs::data_dir());
        return (config, state, data, dirs::runtime_dir(), true);
    }
    (None, None, None, None, false)
}
