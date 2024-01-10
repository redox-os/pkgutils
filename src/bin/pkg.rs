use clap::{App, Arg, SubCommand};
use indexmap::IndexMap;
use pkgutils::{Database, Package, PackageDepends, PackageMeta, PackageMetaList, Repo};
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::{env, process};
use version_compare::{CompOp, VersionCompare};

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
                }
                _ => (),
            },
            Err(_err) => {
                println!(
                    "{}: version parsing error when comparing {} and {}",
                    package, version, remote_version
                );
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
            liner::Prompt::from("Do you want to upgrade these packages? (Y/n) "),
            None,
            &mut liner::BasicCompleter::new(vec!["yes", "no"]),
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
            }
            _ => {
                println!("Cancelling upgrade.");
            }
        }
    }

    Ok(())
}

fn main() {
    let matches = App::new("pkg")
        .about("A package management utility for Redox OS")
        .arg(
            Arg::with_name("target")
                .help("The target architecture")
                .long("target")
                .takes_value(true),
        )
        .subcommand(
            SubCommand::with_name("clean")
                .about("Clean an extracted package")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("create")
                .about("Create a package")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("extract")
                .about("Extract a package")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("fetch")
                .about("Download a package")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Install a package")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("root")
                        .help("The root package directory")
                        .long("root")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List package contents")
                .arg(
                    Arg::with_name("package")
                        .help("The name of the package")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("sign")
                .about("Get a file signature")
                .arg(
                    Arg::with_name("file")
                        .help("The file to obtain the signature of")
                        .multiple(true)
                        .required(true),
                ),
        )
        .subcommand(SubCommand::with_name("upgrade").about("Upgrade all installed packages"))
        .get_matches();

    let target = matches
        .value_of("target")
        .unwrap_or(env!("PKG_DEFAULT_TARGET"));

    let repo = Repo::new(target);
    let database = Database::open("/pkg", PackageDepends::Repository(Repo::new(target)));

    let mut success = true;

    macro_rules! print_result {
        ( $res:expr, $ok_fmt:expr $(, $var:expr )* ) => {
            let res = $res;
            eprint!("pkg: {}: ", matches.subcommand_name().unwrap());
            $( eprint!("{}: ", $var); )*
            match res {
                // {0:.0?} is a hack to avoid "argument never used"
                Ok(res) => eprintln!(concat!("{0:.0?}", $ok_fmt), res),
                Err(err) => {
                    eprint!("failed: {}", err);
                    if let Some(cause) = err.source() {
                        eprint!(" ({})", cause);
                    }
                    eprintln!();
                    success = false;
                }
            }
        }
    }

    match matches.subcommand() {
        ("clean", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                print_result!(repo.clean(package), "cleaned {}", package);
            }
        }
        ("create", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                print_result!(repo.create(package), "created {}", package);
            }
        }
        ("extract", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                print_result!(repo.extract(package), "extracted to {}", package);
            }
        }
        ("fetch", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                let res = repo.fetch(package);
                let res = res.as_ref().map(|p| p.path().display());
                print_result!(res, "fetched {}", package);
            }
        }
        ("install", Some(m)) => {
            let mut dependencies = IndexMap::new();
            let mut tar_gz_pkgs = Vec::new();

            // Calculate dependencies for packages listed in database
            for package in m.values_of("package").unwrap() {
                // Check if package is in current directory
                if package.ends_with(".tar.gz") {
                    let path = env::current_dir().unwrap().join(&package);

                    // Extract package report errors
                    match Package::from_path(&path) {
                        Ok(p) => {
                            tar_gz_pkgs.push(p);
                        }
                        Err(e) => {
                            eprintln!("error during package open: {}", e);
                            if let Some(cause) = e.source() {
                                eprintln!("cause: {}", cause);
                            }
                            success = false;
                        }
                    }
                } else {
                    // Package is not in current directory so calculate dependencies
                    // from database
                    match database.calculate_depends(package, &mut dependencies) {
                        Ok(_) => {
                            dependencies.insert(package.to_string(), ());
                        }
                        Err(e) => {
                            eprintln!("error during dependency calculation: {}", e);
                            if let Some(cause) = e.source() {
                                eprintln!("cause: {}", cause);
                            }
                            success = false;
                        }
                    }
                }
            }

            // Download each package, except *.tar.gz, and then install each package.
            for package in dependencies.keys() {
                let pkg = repo.fetch(package);

                let dest = m.value_of("root").unwrap_or("/");
                print_result!(pkg.and_then(|mut p| p.install(dest)), "succeeded", package);
            }

            for mut package in tar_gz_pkgs {
                let dest = m.value_of("root").unwrap_or("/");
                print_result!(package.install(dest), "succeeded");
            }
        }
        ("list", Some(m)) => {
            for package in m.values_of("package").unwrap() {
                let res = repo.fetch(package).and_then(|mut p| p.list());
                print_result!(res, "succeeded", package);
            }
        }
        ("sign", Some(m)) => {
            for file in m.values_of("file").unwrap() {
                print_result!(repo.signature(file), "{}", file);
            }
        }
        ("upgrade", _) => {
            print_result!(upgrade(repo), "succeeded");
        }
        _ => {
            eprintln!("{}", matches.usage());
            success = false;
        }
    }

    process::exit(if success { 0 } else { 1 });
}
