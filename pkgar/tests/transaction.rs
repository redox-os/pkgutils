use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pkgar::{PackageFile, Transaction};
use pkgar_keys::SecretKeyFile;

struct TestDir {
    tmpdir: tempfile::TempDir,
}

impl TestDir {
    fn new() -> io::Result<TestDir> {
        Ok(TestDir {
            tmpdir: tempfile::tempdir()?,
        })
    }

    fn dir(&self, path: impl AsRef<Path>) -> PathBuf {
        self.tmpdir.path().join(path)
    }

    fn file(&self, path: impl AsRef<Path>) -> PathBuf {
        self.tmpdir.path().join(path)
    }
}

const MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

#[test]
fn build_install_update_remove() -> Result<(), Box<dyn Error>> {
    let tmp = TestDir::new()?;
    fs::create_dir(tmp.dir("keys"))?;

    let (pkey_file, skey_file) = SecretKeyFile::new();
    pkey_file.save(&tmp.file("keys/public.toml"))?;
    skey_file.save(&tmp.file("keys/private.toml"))?;

    let pkgar_src = PathBuf::from(MANIFEST_DIR).join("src");
    println!("Copying {:?} to buildroot", pkgar_src);
    copy_dir::copy_dir(pkgar_src, tmp.dir("buildroot"))?;

    println!("Create archive");
    pkgar::create(
        tmp.file("keys/private.toml"),
        tmp.file("pkgar-src-1.pkgar"),
        tmp.dir("buildroot"),
    )?;

    println!("Read pkgar-src-1.pkgar");
    let mut src_pkg = PackageFile::new(tmp.file("pkgar-src-1.pkgar"), &pkey_file.pkey)?;

    println!("Install archive");
    let mut install = Transaction::install(&mut src_pkg, tmp.dir("installroot"))?;
    install.commit()?;

    println!("Modify build");
    fs::remove_file(tmp.file("buildroot/main.rs"))?;
    pkgar::create(
        tmp.file("keys/private.toml"),
        tmp.file("pkgar-src-2.pkgar"),
        tmp.file("buildroot"),
    )?;

    println!("Read pkgar-src-2.pkgar");
    let mut src2_pkg = PackageFile::new(tmp.file("pkgar-src-2.pkgar"), &pkey_file.pkey)?;

    println!("Upgrade archive");
    let mut update = Transaction::replace(&mut src_pkg, &mut src2_pkg, tmp.dir("installroot"))?;
    update.commit()?;

    println!("Uninstall archive");
    let mut remove = Transaction::remove(&mut src2_pkg, tmp.dir("installroot"))?;
    remove.commit()?;

    //TODO: pacakge directory is left after uninstall assert_eq!(fs::read_dir(tmp.dir("installroot"))?.count(), 0);
    Ok(())
}
