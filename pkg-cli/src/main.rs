use std::{cell::RefCell, io, process, rc::Rc};

use clap::{Parser, Subcommand};
use pkg::{callback::IndicatifCallback, Library, PackageName};
use termion::{color, is_tty, style};

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

// TODO: Refactor this
fn procces_packages(input: Vec<String>, library: &mut Library, all: bool) -> Vec<PackageName> {
    if all {
        match library.get_all_package_names() {
            Ok(packages) => packages,
            Err(_) => {
                eprintln!("Unable to get all packages");
                process::exit(1);
            }
        }
        
    } else {
        let mut packages = vec![];
        for p in input {
            if let Ok(package) = PackageName::new(p) {
                packages.push(package);
            }
        }
        if packages.len() == 0 {
            eprintln!("No packages selected");
            process::exit(1);
        }
        packages
    }
}

fn main() {
    let args = Cli::parse();
    let mut callback = IndicatifCallback::new();
    callback.set_interactive(true);

    let (install_path, target) = if cfg!(target_os = "redox") {
        ("/", env!("TARGET"))
    } else {
        ("/tmp/pkg_install", "x86_64-unknown-redox")
    };

    let mut library = Library::new(install_path, target, Rc::new(RefCell::new(callback)))
        .unwrap_or_else(|err| {
            eprintln!(
                "{}Error: Failed to initialize package library: {:?}{}",
                color::Fg(color::Red),
                err,
                style::Reset
            );
            if matches!(err, pkg::backend::Error::MissingPermissions) {
                eprintln!("Hint: You may need root privileges. Try running with 'sudo'.");
            }
            std::process::exit(1);
        });

    execute_command(args.command, &mut library).unwrap_or_else(|err| {
        if is_tty(&io::stderr()) {
            eprintln!(
                "{}{}error: {}{}{:?}{}",
                color::Fg(color::Red),
                style::Bold,
                style::Reset,
                color::Fg(color::Red),
                err,
                style::Reset
            );
        } else {
            eprintln!("error: {:#?}", err);
        }
        // TODO: this hanging the terminal
        // process::exit(1);
    });
}
fn execute_command(
    command: Commands,
    library: &mut Library,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut needs_apply = false;

    match command {
        Commands::Install { packages, all } => {
            let packages = procces_packages(packages, library, all);
            library.install(packages)?;
            needs_apply = true;
        }
        Commands::Remove { packages, all } => {
            let packages = procces_packages(packages, library, all);
            library.uninstall(packages)?;
            needs_apply = true;
        }
        Commands::Update { packages, all } => {
            let packages = procces_packages(packages, library, all);
            library.update(packages)?;
            needs_apply = true;
        }
        Commands::Search { package } => {
            let packages = library.search(&package)?;
            println!("{:?}", packages);
        }
        Commands::Info { package } => {
            let package = PackageName::new(package)?;
            let info = library.info(package)?;
            println!("{:#?}", info);
        }
        Commands::List => {
            let packages = library.get_installed_packages()?;
            println!("{:#?}", packages);
        }
    }

    if needs_apply {
        if let Err(e) = library.apply() {
            if let Err(e) = library.abort() {
                eprintln!("Cannot aborting: {:#?}", e);
            }
            return Err(e.into());
        }
    }

    Ok(())
}
