#![deny(warnings)]

extern crate hyper;
extern crate hyper_rustls;
extern crate octavo;

use octavo::octavo_digest::Digest;
use octavo::octavo_digest::sha3::Sha512;
use std::str;
use std::fs::{self, File};
use std::io::{self, stderr, Read, Write};
use std::path::Path;
use std::process::Command;

pub use download::download;

mod download;

//TODO: Allow URLs for other archs
pub static REPO_REMOTE: &'static str = "http://static.redox-os.org/pkg/x86_64-unknown-redox";
pub static REPO_LOCAL: &'static str = "/tmp/redox-pkg";

pub fn sync(file: &str) -> io::Result<String> {
    let local_path = format!("{}/{}", REPO_LOCAL, file);
    if Path::new(&local_path).is_file() {
        write!(stderr(), "* Already downloaded {}\n", file)?;
    } else {
        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let remote_path = format!("{}/{}", REPO_REMOTE, file);
        download(&remote_path, &local_path)?;
    }
    Ok(local_path)
}

pub fn signature(file: &str) -> io::Result<String> {
    let mut data = vec![];
    File::open(&file)?.read_to_end(&mut data)?;

    let mut output = vec![0; Sha512::output_bytes()];
    let mut hash = Sha512::default();
    hash.update(&data);
    hash.result(&mut output);

    let mut encoded = String::new();
    for b in output.iter() {
        encoded.push_str(&format!("{:X}", b));
    }

    Ok(encoded)
}

pub fn clean(package: &str) -> io::Result<String> {
    let tardir = format!("{}/{}", REPO_LOCAL, package);
    fs::remove_dir_all(&tardir)?;
    Ok(tardir)
}

pub fn create(package: &str) -> io::Result<String> {
    if ! Path::new(package).is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", package)));
    }

    let sigfile = format!("{}.sig", package);
    let tarfile = format!("{}.tar", package);

    Command::new("tar")
        .arg("cf")
        .arg(&format!("../{}", tarfile))
        .arg(".")
        .current_dir(package)
        .spawn()?
        .wait()?;

    let mut signature = signature(&tarfile)?;
    signature.push('\n');

    File::create(&sigfile)?.write_all(&signature.as_bytes())?;

    Ok(tarfile)
}

pub fn fetch(package: &str) -> io::Result<String> {
    //TODO let sigfile = sync(&format!("{}.sig", package))?;
    let tarfile = sync(&format!("{}.tar", package))?;

    /*TODO Check signature
    let mut expected = String::new();
    File::open(sigfile)?.read_to_string(&mut expected)?;
    if expected.trim() != signature(&tarfile)? {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{} not valid", package)));
    }
    */

    Ok(tarfile)
}

pub fn extract(package: &str) -> io::Result<String> {
    let tarfile = fetch(package)?;
    let tardir = format!("{}/{}", REPO_LOCAL, package);
    fs::create_dir_all(&tardir)?;

    Command::new("tar")
        .arg("xf")
        .arg(&tarfile)
        .current_dir(&tardir)
        .spawn()?
        .wait()?;

    Ok(tardir)
}

pub fn install(package: &str) -> io::Result<()> {
    let tarfile = fetch(package)?;

    let status = Command::new("tar")
        .arg("xf")
        .arg(&tarfile)
        .current_dir("/")
        .spawn()?
        .wait()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "tar command failed"))
    }
}

pub fn list(package: &str) -> io::Result<()> {
    let tarfile = fetch(package)?;

    Command::new("tar")
        .arg("tf")
        .arg(&tarfile)
        .spawn()?
        .wait()?;

    Ok(())
}
