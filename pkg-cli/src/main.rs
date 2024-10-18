use std::{cell::RefCell, rc::Rc};

use clap::{Parser, Subcommand};
use pkg::{callback::IndicatifCallback, Library, PackageName};

/// Redox Package Manager
#[derive(Debug, Parser)]
#[command(name = "pkg")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// install package(s)
    #[command(arg_required_else_help = true)]
    Install {
        /// package(s)
        packages: Vec<String>,

        #[arg(short = 'a')]
        all: bool,
    },

    /// remove package(s)
    #[command(arg_required_else_help = true)]
    Remove {
        /// package(s)
        packages: Vec<String>,

        #[arg(short = 'a')]
        all: bool,
    },

    /// update package(s) if nothing is spesified updates all installed packages
    Update {
        /// package(s)
        packages: Vec<String>,

        #[arg(short = 'a')]
        all: bool,
    },

    /// search for a package
    #[command(arg_required_else_help = true)]
    Search {
        /// package
        package: String,
    },

    /// information about a package
    #[command(arg_required_else_help = true)]
    Info {
        /// package
        package: String,
    },

    /// list installed packages
    List,
}

fn procces_packages(input: Vec<String>, library: &mut Library, all: bool) -> Vec<PackageName> {
    let mut packages = vec![];
    let all_packages = library.get_all_package_names().unwrap();

    if all {
        return all_packages;
    }

    for pattern_string in input.iter() {
        let pattern = glob::Pattern::new(pattern_string).unwrap();

        for package in all_packages.iter() {
            if pattern.matches(package.as_str()) {
                packages.push(package.clone());
            }
        }
    }

    packages
}

fn main() {
    let callback = IndicatifCallback::new();

    let (install_path, target) = if cfg!(target_os = "redox") {
        ("/", env!("TARGET"))
    } else {
        ("/tmp/pkg_install", "x86_64-unknown-redox")
    };

    let mut library = Library::new(install_path, target, Rc::new(RefCell::new(callback))).unwrap();

    let args = Cli::parse();

    match args.command {
        Commands::Install { packages, all } => {
            let packages = procces_packages(packages, &mut library, all);
            library.install(packages).unwrap();
        }
        Commands::Remove { packages, all } => {
            let packages = procces_packages(packages, &mut library, all);
            library.uninstall(packages).unwrap();
        }
        Commands::Update { packages, all } => {
            let packages = procces_packages(packages, &mut library, all);
            library.update(packages).unwrap();
        }
        Commands::Search { package } => {
            let packages = library.search(&package).unwrap();
            println!("{:?}", packages);
            return;
        }
        Commands::Info { package } => {
            let package = PackageName::new(package).unwrap();
            let info = library.info(package).unwrap();
            println!("{:#?}", info);
            return;
        }
        Commands::List => {
            let packages = library.get_installed_packages().unwrap();
            println!("{:#?}", packages);
            return;
        }
    }

    library.apply().unwrap();
    println!("done");
}
