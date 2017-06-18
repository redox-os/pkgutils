#![deny(warnings)]

extern crate liner;
extern crate pkgutils;
extern crate version_compare;

use pkgutils::{Repo, Package, PackageMeta, PackageMetaList};
use std::{env, process};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use version_compare::{VersionCompare, CompOp};

fn upgrade(repo: Repo) -> io::Result<()> {
    let mut local_list = PackageMetaList::new();
    if Path::new("/pkg/").is_dir() {
        for entry_res in fs::read_dir("/pkg/")? {
            let entry = entry_res?;

            let mut toml = String::new();
            File::open(entry.path())?.read_to_string(&mut toml)?;

            if let Ok(package) = PackageMeta::from_toml(&toml) {
                local_list.packages.insert(package.name, package.version);
            }
        }
    }

    let tomlfile = repo.sync("repo.toml")?;

    let mut toml = String::new();
    File::open(tomlfile)?.read_to_string(&mut toml)?;

    let remote_list = PackageMetaList::from_toml(&toml).map_err(|err| {
        io::Error::new(io::ErrorKind::InvalidData, format!("TOML error: {}", err))
    })?;

    let mut upgrades = Vec::new();
    for (package, version) in local_list.packages.iter() {
        let remote_version = remote_list.packages.get(package).map_or("", |s| &s);
        match VersionCompare::compare(version, remote_version) {
            Ok(cmp) => match cmp {
                CompOp::Lt => {
                    upgrades.push((package.clone(), version.clone(), remote_version.to_string()));
                },
                _ => ()
            },
            Err(_err) => {
                println!("{}: version parsing error when comparing {} and {}", package, version, remote_version);
            }
        }
    }

    if upgrades.is_empty() {
        println!("All packages are up to date.");
    } else {
        for &(ref package, ref old_version, ref new_version) in upgrades.iter() {
            println!("{}: {} => {}", package, old_version, new_version);
        }

        let line = liner::Context::new().read_line(
            "Do you want to upgrade these packages? (Y/n) ",
            &mut |_| {}
        )?;
        match line.to_lowercase().as_str() {
            "" | "y" | "yes" => {
                println!("Downloading packages");
                let mut packages = Vec::new();
                for (package, _, _) in upgrades {
                    packages.push(repo.fetch(&package)?);
                }

                println!("Installing packages");
                for mut package in packages {
                    package.install("/")?;
                }
            },
            _ => {
                println!("Cancelling upgrade.");
            }
        }
    }

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
    write!(io::stderr(), "    upgrade - upgrade all packages\n")?;

    Ok(())
}

fn main() {
    let repo = Repo::new(env!("TARGET"));

    let mut args = env::args().skip(1);
    if let Some(op) = args.next() {
        match op.as_str() {
            "clean" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        match repo.clean(package) {
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
                        match repo.create(package) {
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
                        match repo.extract(package) {
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
                        match repo.fetch(package) {
                            Ok(pkg) => {
                                let _ = write!(io::stderr(), "pkg: fetch: {}: fetched {}\n", package, pkg.path().display());
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
                        let pkg = if package.ends_with(".tar.gz") {
                            let path = format!("{}/{}", env::current_dir().unwrap().to_string_lossy(), package);
                            Package::from_path(&path)
                        } else {
                            repo.fetch(package)
                        };

                        if let Err(err) = pkg.and_then(|mut p| p.install("/")) {
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
                        if let Err(err) = repo.fetch(package).and_then(|mut p| p.list()) {
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
                        match repo.signature(file) {
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
            "upgrade" => {
                match upgrade(repo) {
                    Ok(()) => {
                        let _ = write!(io::stderr(), "pkg: upgrade: succeeded\n");
                    },
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: upgrade: failed: {}\n", err);
                    }
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
