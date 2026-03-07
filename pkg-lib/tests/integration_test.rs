use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "indicatif")]
use pkg::callback::IndicatifCallback;

#[cfg(not(feature = "indicatif"))]
use pkg::callback::PlainCallback;

use pkg::{Library, PackageName, PackageState};

#[test]
fn test_pkg_install() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    #[cfg(feature = "indicatif")]
    let callback = IndicatifCallback::new();

    #[cfg(not(feature = "indicatif"))]
    let callback = PlainCallback::new();

    let tmp_dir = std::env::current_dir()?.join("tests/staging_install");

    // Simulate redox OS pkg library
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    let pkg_d_dir = tmp_dir.join("etc/pkg.d");
    fs::create_dir_all(&pkg_d_dir)?;
    let source_file = pkg_d_dir.join("50_redox");
    fs::write(&source_file, "https://static.redox-os.org/pkg")?;

    fs::create_dir_all(&tmp_dir)?;

    let mut library = Library::new(
        tmp_dir,
        "x86_64-unknown-redox",
        Rc::new(RefCell::new(callback)),
    )?;

    // ncurses has terminfo
    let list = vec![PackageName::new("ncurses")?];
    library.install(list)?;
    library.apply()?;

    assert_eq!(library.get_installed_packages().unwrap().len(), 2);

    let list = vec![PackageName::new("ncurses")?];
    library.uninstall(list)?;
    library.apply()?;

    assert_eq!(library.get_installed_packages().unwrap().len(), 0);

    Ok(())
}

#[test]
fn test_pkg_update() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    #[cfg(feature = "indicatif")]
    let callback = Rc::new(RefCell::new(IndicatifCallback::new()));

    #[cfg(not(feature = "indicatif"))]
    let callback = Rc::new(RefCell::new(PlainCallback::new()));

    let tmp_dir = std::env::current_dir()?.join("tests/staging_update");

    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let mut library = Library::new_remote(
        &vec!["https://static.redox-os.org/pkg"],
        tmp_dir.clone(),
        "x86_64-unknown-redox",
        callback.clone(),
    )?;

    // ncurses has terminfo
    let list = vec![PackageName::new("ncurses")?];
    library.install(list)?;
    library.apply()?;

    assert_eq!(library.get_installed_packages().unwrap().len(), 2);

    // should have no update
    library.update(library.get_installed_packages().unwrap())?;
    library.apply()?;

    // force invalidation
    let pkg_path = tmp_dir.join("etc/pkg/packages.toml");
    let file = fs::read_to_string(&pkg_path)?;
    let mut p = PackageState::from_toml(&file)?;
    p.installed.get_mut("ncurses").unwrap().blake3 = "invalid".into();
    fs::write(&pkg_path, p.to_toml())?;
    // reload metadata
    library = Library::new_remote(
        &vec!["https://static.redox-os.org/pkg"],
        tmp_dir,
        "x86_64-unknown-redox",
        callback.clone(),
    )?;

    // should have one update
    library.update(vec![PackageName::new("ncurses")?])?;
    library.apply()?;

    Ok(())
}
