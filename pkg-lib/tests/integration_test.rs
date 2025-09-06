use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "indicatif")]
use pkg::callback::IndicatifCallback;

#[cfg(not(feature = "indicatif"))]
use pkg::callback::PlainCallback;

use pkg::{Library, PackageName};

#[test]
fn test_pkg_install() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    #[cfg(feature = "indicatif")]
    let callback = IndicatifCallback::new();

    #[cfg(not(feature = "indicatif"))]
    let callback = PlainCallback::new();

    let tmp_dir = std::env::current_dir()?.join("tests/staging");

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

    let list = vec![PackageName::new("bootloader")?];
    library.install(list)?;
    library.apply()?;

    let list = vec![PackageName::new("bootloader")?];
    library.uninstall(list)?;
    library.apply()?;

    Ok(())
}
