#![deny(warnings)]

extern crate hyper;
extern crate hyper_rustls;
extern crate octavo;
extern crate tar;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use octavo::octavo_digest::Digest;
use octavo::octavo_digest::sha3::Sha512;
use tar::{Archive, Header};
use std::str;
use std::fs::{self, File};
use std::io::{self, stderr, Read, Write};
use std::path::Path;
use std::io::Cursor;

pub use download::download;
use packagemeta::PackageMeta;

mod download;
mod packagemeta;

pub struct Repo {
    local: String,
    remotes: Vec<String>,
    dest: String,
    target: String,
}

impl Repo {
    pub fn new(target: &str) -> Repo {
        let mut remotes = vec![];

        //TODO: Cleanup
        // This will add every line in every file in /etc/pkg.d to the remotes,
        // provided it does not start with #
        {
            let mut entries = vec![];
            if let Ok(read_dir) = fs::read_dir("/etc/pkg.d") {
                for entry_res in read_dir {
                    if let Ok(entry) = entry_res {
                        let path = entry.path();
                        if path.is_file() {
                            entries.push(path);
                        }
                    }
                }
            }

            entries.sort();

            for entry in entries {
                if let Ok(mut file) = File::open(entry) {
                    let mut data = String::new();
                    if let Ok(_) = file.read_to_string(&mut data) {
                        for line in data.lines() {
                            if ! line.starts_with('#') {
                                remotes.push(line.to_string());
                            }
                        }
                    }
                }
            }
        }

        Repo {
            local: format!("/tmp/redox-pkg"),
            remotes: remotes,
            dest: "/".to_string(),
            target: target.to_string()
        }
    }

    pub fn sync(&self, file: &str) -> io::Result<String> {
        let local_path = format!("{}/{}", self.local, file);
        if Path::new(&local_path).is_file() {
            write!(stderr(), "* Already downloaded {}\n", file)?;
            Ok(local_path)
        } else {
            if let Some(parent) = Path::new(&local_path).parent() {
                fs::create_dir_all(parent)?;
            }

            let mut res = Err(io::Error::new(io::ErrorKind::NotFound, format!("no remote paths")));
            for remote in self.remotes.iter() {
                let remote_path = format!("{}/{}/{}", remote, self.target, file);
                res = download(&remote_path, &local_path).map(|_| local_path.clone());
                if res.is_ok() {
                    break;
                }
            }
            res
        }
    }

    pub fn signature(&self, file: &str) -> io::Result<String> {
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

    pub fn clean(&self, package: &str) -> io::Result<String> {
        let tardir = format!("{}/{}", self.local, package);
        fs::remove_dir_all(&tardir)?;
        Ok(tardir)
    }

    pub fn create(&self, dir: &str, package: &str, version: &str) -> io::Result<String> {
        if ! Path::new(dir).is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", dir)));
        }

        let metadata = PackageMeta::new(package, version, &self.target).to_toml();

        let sigfile = format!("{}.sig", dir);
        let tarfile = format!("{}.tar", dir);

        {
            let file = File::create(&tarfile)?;
            let mut tar = tar::Builder::new(file);

            tar.append_dir_all("", dir)?;

            let mut header = Header::new_gnu();
            header.set_path(&format!("etc/pkg.d/{}.toml", package))?;
            header.set_size(metadata.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, Cursor::new(metadata))?;

            tar.finish()?;
        }

        let mut signature = self.signature(&tarfile)?;
        signature.push('\n');

        File::create(&sigfile)?.write_all(&signature.as_bytes())?;

        Ok(tarfile)
    }

    pub fn fetch(&self, package: &str) -> io::Result<String> {
        //TODO let sigfile = sync(&format!("{}.sig", package))?;
        let tarfile = self.sync(&format!("{}.tar", package))?;

        /*TODO Check signature
        let mut expected = String::new();
        File::open(sigfile)?.read_to_string(&mut expected)?;
        if expected.trim() != signature(&tarfile)? {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{} not valid", package)));
        }
        */

        Ok(tarfile)
    }

    pub fn extract(&self, package: &str) -> io::Result<String> {
        let tarfile = self.fetch(package)?;
        let tardir = format!("{}/{}", self.local, package);
        fs::create_dir_all(&tardir)?;

        let mut ar = Archive::new(File::open(&tarfile)?);
        ar.set_preserve_permissions(true);
        ar.unpack(&tardir)?;

        Ok(tardir)
    }

    pub fn install_file(&self, path: &str)-> io::Result<()> {
        let mut ar = Archive::new(File::open(path)?);
        ar.set_preserve_permissions(true);
        ar.unpack(&self.dest)?;
        Ok(())
    }

    pub fn install(&self, package: &str) -> io::Result<()> {
        let tarfile = self.fetch(package)?;
        self.install_file(&tarfile)
    }

    pub fn list(&self, package: &str) -> io::Result<()> {
        let tarfile = self.fetch(package)?;

        let mut ar = Archive::new(File::open(tarfile)?);
        for i in ar.entries()? {
            println!("{}", i?.path()?.display());
        }

        Ok(())
    }

    pub fn set_dest(&mut self, dest: &str) {
        self.dest = dest.to_string();
    }

    pub fn add_remote(&mut self, remote: &str) {
        self.remotes.push(remote.to_string());
    }
}
