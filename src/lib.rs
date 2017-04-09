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

    pub fn create(&self, package: &str) -> io::Result<String> {
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

        Command::new("tar")
            .arg("xf")
            .arg(&tarfile)
            .current_dir(&tardir)
            .spawn()?
            .wait()?;

        Ok(tardir)
    }

    pub fn install_file(&self, path: &str)-> io::Result<()> {
        let status = Command::new("tar")
            .arg("xf")
            .arg(path)
            .current_dir(&self.dest)
            .spawn()?
            .wait()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "tar command failed"))
        }
    }

    pub fn install(&self, package: &str) -> io::Result<()> {
        let tarfile = self.fetch(package)?;
        self.install_file(&tarfile)
    }

    pub fn list(&self, package: &str) -> io::Result<()> {
        let tarfile = self.fetch(package)?;

        Command::new("tar")
            .arg("tf")
            .arg(&tarfile)
            .spawn()?
            .wait()?;

        Ok(())
    }

    pub fn set_dest(&mut self, dest: &str) {
        self.dest = dest.to_string();
    }

    pub fn add_remote(&mut self, remote: &str) {
        self.remotes.push(remote.to_string());
    }
}
