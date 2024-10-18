use std::{cell::RefCell, rc::Rc};

use clap::{Parser, Subcommand};
use pkg::{net_backend::Callback, *};

use indicatif::{ProgressBar, ProgressStyle};

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

#[derive(Clone)]
struct BasicCallback {
    pb: ProgressBar,
}

impl Callback for BasicCallback {
    fn start_download(&mut self, length: u64, file: &str) {
        self.pb = ProgressBar::new(length);
        self.pb.set_style(ProgressStyle::with_template("{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));

        // do actual parsing not this
        let mut msg = file.replace("https://static.redox-os.org/pkg/", "");
        msg = msg.replace("x86_64-unknown-redox/", "");

        self.pb.set_message(msg);
    }

    fn increment_downloaded(&mut self, downloaded: usize) {
        self.pb.inc(downloaded as u64);
    }
    fn end_download(&mut self) {
        self.pb.finish();
        println!();
    }
}

fn procces_packages(input: Vec<String>, library: &mut Library, all: bool) -> Vec<String> {
    let mut packages = vec![];
    let all_packages = library.get_all_package_names().unwrap();

    if all {
        return all_packages;
    }

    for pattern_string in input.iter() {
        let patern = glob::Pattern::new(pattern_string).unwrap();

        for package in all_packages.iter() {
            if patern.matches(package) {
                packages.push(package.clone());
            }
        }
    }

    packages
}

fn main() {
    // std::env::set_var("RUST_BACKTRACE", "1");

    let cli = BasicCallback {
        pb: ProgressBar::hidden(),
    };

    let mut library = Library::new(Rc::new(RefCell::new(cli))).unwrap();

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
