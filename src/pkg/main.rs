extern crate octavo;

use octavo::octavo_digest::Digest;
use octavo::octavo_digest::sha3::Sha512;
use std::{env, str};
use std::fs::{self, File};
use std::io::{self, stderr, Read, Write};
use std::path::Path;
use std::process::{self, Command};

use download::download;

mod download;

//TODO: Allow URLs for other archs
static REPO_REMOTE: &'static str = "http://static.redox-os.org/pkg/x86_64-unknown-redox";
static REPO_LOCAL: &'static str = "/tmp/redox-pkg";

fn sync(file: &str) -> io::Result<String> {
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

fn signature(file: &str) -> io::Result<String> {
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

fn clean(package: &str) -> io::Result<String> {
    let tardir = format!("{}/{}", REPO_LOCAL, package);
    fs::remove_dir_all(&tardir)?;
    Ok(tardir)
}

fn create(package: &str) -> io::Result<String> {
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

fn fetch(package: &str) -> io::Result<String> {
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

fn extract(package: &str) -> io::Result<String> {
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

fn install(package: &str) -> io::Result<()> {
    let tarfile = fetch(package)?;

    Command::new("tar")
        .arg("xf")
        .arg(&tarfile)
        .current_dir("/")
        .spawn()?
        .wait()?;

    Ok(())
}

fn list(package: &str) -> io::Result<()> {
    let tarfile = fetch(package)?;

    Command::new("tar")
        .arg("tf")
        .arg(&tarfile)
        .spawn()?
        .wait()?;

    Ok(())
}

fn help() -> io::Result<()> {
    write!(io::stderr(), "pkg [command] [arguments]\n")?;
    write!(io::stderr(), "    clean [package] - clean an extracted package\n")?;
    write!(io::stderr(), "    create [directory] - create a package\n")?;;
    write!(io::stderr(), "    extract [package] - extract a package\n")?;
    write!(io::stderr(), "    fetch [package] - download a package\n")?;
    write!(io::stderr(), "    help - show this help message\n")?;
    write!(io::stderr(), "    install [package] - install a package\n")?;
    write!(io::stderr(), "    list [package] - list package contents\n")?;
    write!(io::stderr(), "    sign [file] - get a file signature\n")?;

    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);
    if let Some(op) = args.next() {
        match op.as_str() {
            "clean" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        match clean(package) {
                            Ok(tardir) => {
                                let _ = write!(io::stderr(), "pkg: clean: {}: cleaned {}\n", package, tardir);
                            }
                            Err(err) => {
                                let _ = write!(io::stderr(), "pkg: clean: {}: failed: {}\n", package, err);
                            }
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: clean: no packages specified\n");
                    process::exit(1);
                }
            },
            "create" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        match create(package) {
                            Ok(tarfile) => {
                                let _ = write!(io::stderr(), "pkg: create: {}: created {}\n", package, tarfile);
                            }
                            Err(err) => {
                                let _ = write!(io::stderr(), "pkg: create: {}: failed: {}\n", package, err);
                            }
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: create: no packages specified\n");
                    process::exit(1);
                }
            },
            "extract" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        match extract(package) {
                            Ok(tardir) => {
                                let _ = write!(io::stderr(), "pkg: extract: {}: extracted to {}\n", package, tardir);
                            },
                            Err(err) => {
                                let _ = write!(io::stderr(), "pkg: extract: {}: failed: {}\n", package, err);
                            }
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: extract: no packages specified\n");
                    process::exit(1);
                }
            },
            "fetch" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        match fetch(package) {
                            Ok(tarfile) => {
                                let _ = write!(io::stderr(), "pkg: fetch: {}: fetched {}\n", package, tarfile);
                            },
                            Err(err) => {
                                let _ = write!(io::stderr(), "pkg: fetch: {}: failed: {}\n", package, err);
                            }
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: fetch: no packages specified\n");
                    process::exit(1);
                }
            },
            "help" => {
                let _ = help();
            },
            "install" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        if let Err(err) = install(package) {
                            let _ = write!(io::stderr(), "pkg: install: {}: failed: {}\n", package, err);
                        } else {
                            let _ = write!(io::stderr(), "pkg: install: {}: succeeded\n", package);
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: install: no packages specified\n");
                    process::exit(1);
                }
            },
            "list" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        if let Err(err) = list(package) {
                            let _ = write!(io::stderr(), "pkg: list: {}: failed: {}\n", package, err);
                        } else {
                            let _ = write!(io::stderr(), "pkg: list: {}: succeeded\n", package);
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: list: no packages specified\n");
                    process::exit(1);
                }
            },
            "sign" => {
                let files: Vec<String> = args.collect();
                if ! files.is_empty() {
                    for file in files.iter() {
                        match signature(file) {
                            Ok(signature) => {
                                let _ = write!(io::stderr(), "pkg: sign: {}: {}\n", file, signature);
                            },
                            Err(err) => {
                                let _ = write!(io::stderr(), "pkg: sign: {}: failed: {}\n", file, err);
                            }
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: sign: no files specified\n");
                    process::exit(1);
                }
            },
            _ => {
                let _ = write!(io::stderr(), "pkg: {}: unknown operation\n", op);
                let _ = help();
                process::exit(1);
            }
        }
    } else {
        let _ = write!(io::stderr(), "pkg: no operation\n");
        let _ = help();
        process::exit(1);
    }
}
