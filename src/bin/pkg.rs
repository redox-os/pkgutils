#![deny(warnings)]

extern crate pkgutils;

use pkgutils::Repo;
use std::{env, process};
use std::io::{self, Write};

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
                        if let Err(err) = repo.install(package) {
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
                        if let Err(err) = repo.list(package) {
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
