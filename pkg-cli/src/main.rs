use std::{cell::RefCell, io, process, rc::Rc};

use clap::{Parser, Subcommand};
use pkg::{
    backend::Error,
    callback::IndicatifCallback,
    net_backend::{CurlBackend, DownloadBackend, ReqwestBackend},
    Library, LibraryBuilder, PackageName, RepoManager,
};
use termion::{color, is_tty, style};
mod cpu;

/// Redox Package Manager
#[derive(Clone, Debug, Parser)]
#[command(name = "pkg")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    // these are optional configuration that will be prompted when needed
    /// Use cURL backend to perform download
    #[arg(long, global = true)]
    curl: bool,
    /// Always reply with yes even with interactive tty
    #[arg(long, global = true)]
    yes: bool,
    /// Do not try to preserve locally changed files
    #[arg(long, global = true)]
    nocheck: bool,
}

#[derive(Clone, Debug, Subcommand)]
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

    /// Test if remote repository is working
    Test,

    /// Cpu eligibility test
    CpuTest,
}

// TODO: Refactor this
fn process_packages(input: Vec<String>, library: &mut Library, all: bool) -> Vec<PackageName> {
    if all {
        match library.get_all_package_names() {
            Ok(packages) => packages,
            Err(err) => {
                eprintln!("Unable to get all packages: {err}");
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
    let mut args = Cli::parse();
    let mut callback = IndicatifCallback::new();

    let (install_path, target) = if cfg!(target_os = "redox") {
        ("/", env!("TARGET"))
    } else {
        ("/tmp/pkg_install", "x86_64-unknown-redox")
    };
    let color_support_stdout = is_tty(&io::stdout());
    let color_support_stderr = is_tty(&io::stderr());
    callback.set_interactive(color_support_stdout);
    if !args.yes {
        // TODO: --pedantic to enable Some(false)
        callback.set_always_yes(None);
    }
    let mut library =
        LibraryBuilder::new(install_path).with_callback(Rc::new(RefCell::new(callback)));
    if args.nocheck {
        library = library.with_nocheck(args.nocheck)
    }

    let err = loop {
        let net_library = library.clone_with_net_backend(if args.curl {
            Box::new(CurlBackend::new().unwrap())
        } else {
            Box::new(ReqwestBackend::new().unwrap())
        });
        match execute_command(args.clone(), net_library, target, color_support_stdout) {
            Ok(_) => break Ok(()),
            e @ Err(Error::MissingPermissions) => break e,
            Err(err @ Error::Download(_)) if !args.curl => {
                // it doesn't feel right to retry when using --yes
                if !color_support_stdout || args.yes {
                    break Err(err);
                }
                report_error(color_support_stderr, &err);
                eprintln!("Do you want to retry with curl? [Y/n]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap_or(0);
                let input = input.trim().to_lowercase();
                if input == "n" || input == "no" {
                    break Err(Error::Interrupted);
                }
                args.curl = true
            }
            Err(e) => break Err(e),
        }
    };
    if let Err(err) = err {
        report_error(color_support_stderr, &err);
        if matches!(err, pkg::backend::Error::MissingPermissions) {
            // TODO: ask to rerun as sudo
            eprintln!("Hint: You may need root privileges. Try running with 'sudo'.");
        }
        // TODO: this hanging the terminal
        // process::exit(1);
    }
}

fn report_error(color_support_stderr: bool, err: &Error) {
    if color_support_stderr {
        eprintln!(
            "{}{}error: {}{}{}{}",
            color::Fg(color::Red),
            style::Bold,
            style::Reset,
            color::Fg(color::Red),
            *err,
            style::Reset
        );
    } else {
        eprintln!("error: {}", *err);
    }
}
fn execute_command(
    cli: Cli,
    library: LibraryBuilder,
    target: &str,
    color_support: bool,
) -> Result<(), Error> {
    let mut needs_apply = false;
    let install_path = library.install_path();
    match cli.command {
        Commands::Test => {
            let mut r: RepoManager = library.try_into()?;
            r.test_sync_keys()?;
            eprintln!("OK");
            return Ok(());
        }
        Commands::CpuTest => {
            let d = cpu::CpuDetection::probe();
            eprintln!("Eligible cpu level: {:?}", d.eligible);
            if let Some(n) = d.next_eligible() {
                eprintln!(
                    "Next cpu level requirement for {:?}: {:?}",
                    n,
                    d.unsatisfied_extensions(n)
                );
            }
            return Ok(());
        }
        _ => {}
    }
    let mut library =
        Library::new_with_builder(library, |r| r.update_remotes(target, &install_path))?;
    let library = &mut library;
    match cli.command {
        Commands::Install { packages, all } => {
            let packages = process_packages(packages, library, all);
            library.install(packages)?;
            needs_apply = true;
        }
        Commands::Remove { packages, all } => {
            let packages = process_packages(packages, library, all);
            library.uninstall(packages)?;
            needs_apply = true;
        }
        Commands::Update { packages, all } => {
            let empty = packages.is_empty();
            let packages = process_packages(packages, library, all || empty);
            library.update(packages)?;
            needs_apply = true;
        }
        Commands::Search { package } => {
            let packages = library.search(&package)?;
            for (i, (name, _)) in packages.iter().enumerate() {
                write_package(i, name, color_support);
            }
        }
        Commands::Info { package } => {
            let package = PackageName::new(package)?;
            let info = library.info(package)?;
            println!("{}", info);
        }
        Commands::List => {
            let packages = library.get_installed_packages()?;
            for (i, name) in packages.iter().enumerate() {
                write_package(i, name, color_support);
            }
        }
        Commands::Test => unreachable!(),
        Commands::CpuTest => unreachable!(),
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
fn write_package(index: usize, name: &PackageName, color_support: bool) {
    if color_support {
        println!(
            "{}{}{}: {}",
            color::Fg(color::LightGreen),
            index + 1,
            style::Reset,
            name,
        );
    } else {
        println!("{}: {}", index + 1, name);
    }
}
