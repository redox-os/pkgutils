#![deny(warnings)]

extern crate liner;
extern crate pkgutils;
extern crate version_compare;
extern crate clap;

use pkgutils::{Repo, Package, PackageMeta, PackageMetaList};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use version_compare::{VersionCompare, CompOp};
use clap::{App, SubCommand, Arg};

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

fn main() {
    let matches = App::new("pkg")
        .arg(Arg::with_name("target")
             .long("target")
             .takes_value(true)
        ).subcommand(SubCommand::with_name("clean")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("create")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("extract")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("fetch")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("install")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("list")
            .arg(Arg::with_name("package")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("sign")
            .arg(Arg::with_name("file")
                 .multiple(true)
                 .required(true)
            )
        ).subcommand(SubCommand::with_name("upgrade")
        ).get_matches();

    let target = matches.value_of("target")
        .or(option_env!("TARGET"))
        .expect(concat!("pkg was not compiled with a target, ",
                        "and --target was not specified"));

    let repo = Repo::new(target);

    match matches.subcommand() {
        ("clean", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                match repo.clean(package) {
                    Ok(tardir) => {
                        let _ = write!(io::stderr(), "pkg: clean: {}: cleaned {}\n", package, tardir);
                    }
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: clean: {}: failed: {}\n", package, err);
                    }
                }
            }
        }
        ("create", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                match repo.create(package) {
                    Ok(tarfile) => {
                        let _ = write!(io::stderr(), "pkg: create: {}: created {}\n", package, tarfile);
                    }
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: create: {}: failed: {}\n", package, err);
                    }
                }
            }
        }
        ("extract", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                match repo.extract(package) {
                    Ok(tardir) => {
                        let _ = write!(io::stderr(), "pkg: extract: {}: extracted to {}\n", package, tardir);
                    },
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: extract: {}: failed: {}\n", package, err);
                    }
                }
            }
        }
        ("fetch", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                match repo.fetch(package) {
                    Ok(pkg) => {
                        let _ = write!(io::stderr(), "pkg: fetch: {}: fetched {}\n", package, pkg.path().display());
                    },
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: fetch: {}: failed: {}\n", package, err);
                    }
                }
            }
        }
        ("install", Some(m)) => {
            for package in m.values_of("package").unwrap() {
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
        }
        ("list", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                if let Err(err) = repo.fetch(package).and_then(|mut p| p.list()) {
                    let _ = write!(io::stderr(), "pkg: list: {}: failed: {}\n", package, err);
                } else {
                    let _ = write!(io::stderr(), "pkg: list: {}: succeeded\n", package);
                }
            }
        }
        ("sign", Some(m)) => {
            for file in m.values_of("file").unwrap() {
                match repo.signature(file) {
                    Ok(signature) => {
                        let _ = write!(io::stderr(), "pkg: sign: {}: {}\n", file, signature);
                    },
                    Err(err) => {
                        let _ = write!(io::stderr(), "pkg: sign: {}: failed: {}\n", file, err);
                    }
                }
            }
        }
        ("upgrade", _) => {
            match upgrade(repo) {
                Ok(()) => {
                    let _ = write!(io::stderr(), "pkg: upgrade: succeeded\n");
                },
                Err(err) => {
                    let _ = write!(io::stderr(), "pkg: upgrade: failed: {}\n", err);
                }
            }
        }
        _ => unreachable!()
    }
}
