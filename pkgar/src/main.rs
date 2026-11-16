use clap::{Arg, ArgAction, Command};
use clap::{crate_authors, crate_description, crate_name, crate_version};
use pkgar::{
    Error, cat, create_with_flags, extract, extract_with_subpath, list, remove,
    remove_with_subpath, replace, replace_with_subpath, split, verify,
};
use pkgar_keys::{DEFAULT_PUBKEY, DEFAULT_SECKEY};

fn cli() -> Result<(), Error> {
    let (default_pkey, default_skey) = (DEFAULT_PUBKEY.as_os_str(), DEFAULT_SECKEY.as_os_str());

    let arg_pkey = Arg::new("pkey")
        .help("Public key file")
        .short('p')
        .long("pkey")
        .global(true)
        .value_name("FILE")
        .default_value(default_pkey);

    let arg_skey = Arg::new("skey")
        .help("Secret key file")
        .short('s')
        .long("skey")
        .required(true)
        .value_name("FILE")
        .default_value(default_skey);

    let arg_archive = Arg::new("archive")
        .help("Archive file")
        .short('a')
        .long("archive")
        .global(true)
        .value_name("FILE");

    let arg_old_pkey = Arg::new("old-pkey")
        .help("Old Public key file (defaults to pkey)")
        .long("old-pkey")
        .value_name("FILE");

    let arg_old_archive = Arg::new("old-archive")
        .help("Old Archive file")
        .long("old-archive")
        .required(true)
        .value_name("FILE");

    let arg_basedir = Arg::new("basedir")
        .help("Directory to unpack to (defaults to '.')")
        .required(true)
        .value_name("DIR")
        .default_value(".");

    let arg_compress = Arg::new("compress")
        .help("Enable compression for the archive")
        .short('c')
        .long("compress")
        .action(ArgAction::SetTrue);

    let arg_long = Arg::new("long")
        .help("Use long list format")
        .short('l')
        .long("list")
        .action(ArgAction::SetTrue);

    let arg_filter = Arg::new("filter")
        .long("filter")
        .help("Path to filter entries")
        .value_name("DIR");

    let arg_filter_strip = Arg::new("strip")
        .help("Strip prefix by filter path")
        .long("strip")
        .action(ArgAction::SetTrue);

    let mut cmd = Command::new(crate_name!())
        .author(crate_authors!(", "))
        .about(crate_description!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(&arg_pkey)
        .arg(&arg_archive)
        .subcommand(
            Command::new("create")
                .about("Create archive")
                .arg(&arg_skey)
                .arg(&arg_basedir)
                .arg(&arg_compress),
        )
        .subcommand(
            Command::new("extract")
                .about("Extract archive")
                .arg(&arg_basedir)
                .arg(&arg_filter)
                .arg(&arg_filter_strip),
        )
        .subcommand(
            Command::new("list")
                .about("List archive")
                .arg(&arg_long)
                .arg(&arg_filter),
        )
        .subcommand(
            Command::new("cat").about("Read an archive entry").arg(
                Arg::new("entry")
                    .help("Path to an entry")
                    .required(true)
                    .value_name("FILE"),
            ),
        )
        .subcommand(
            Command::new("replace")
                .about("Replace old archive")
                .arg(&arg_old_pkey)
                .arg(&arg_old_archive)
                .arg(&arg_basedir)
                .arg(&arg_filter)
                .arg(&arg_filter_strip),
        )
        .subcommand(
            Command::new("remove")
                .about("Unextract archive")
                .arg(&arg_basedir)
                .arg(&arg_filter)
                .arg(&arg_filter_strip),
        )
        .subcommand(
            Command::new("split")
                .about("Split archive into head and data files")
                .arg(
                    Arg::new("head")
                        .help("Header file")
                        .required(true)
                        .value_name("head"),
                )
                .arg(Arg::new("data").help("Data file").value_name("data")),
        )
        .subcommand(
            Command::new("verify")
                .about("Verify archive")
                .arg(&arg_basedir),
        );

    let matches = cmd.clone().get_matches();

    let pkey_path = matches.get_one::<String>("pkey").unwrap().as_str();
    let archive_path = match matches.get_one::<String>("archive") {
        Some(s) => s.as_str(),
        None => {
            cmd.error(
                clap::error::ErrorKind::MissingRequiredArgument,
                "missing --archive <FILE>",
            )
            .exit();
        }
    };

    match matches.subcommand() {
        Some(("create", sub_matches)) => create_with_flags(
            sub_matches.get_one::<String>("skey").unwrap().as_str(),
            archive_path,
            sub_matches.get_one::<String>("basedir").unwrap().as_str(),
            pkgar_core::HeaderFlags::latest(
                pkgar_core::Architecture::Independent,
                match sub_matches.get_flag("compress") {
                    true => pkgar_core::Packaging::LZMA2,
                    false => pkgar_core::Packaging::Uncompressed,
                },
            ),
        ),
        Some(("extract", sub_matches)) => {
            if let Some(filter) = sub_matches.get_one::<String>("filter") {
                extract_with_subpath(
                    pkey_path,
                    archive_path,
                    sub_matches.get_one::<String>("basedir").unwrap().as_str(),
                    filter.as_str(),
                    sub_matches.get_flag("strip"),
                )
            } else {
                extract(
                    pkey_path,
                    archive_path,
                    sub_matches.get_one::<String>("basedir").unwrap().as_str(),
                )
            }
        }
        Some(("replace", sub_matches)) => {
            let old_archive = sub_matches
                .get_one::<String>("old-archive")
                .unwrap()
                .as_str();
            let old_pkey = sub_matches
                .get_one::<String>("old-pkey")
                .unwrap_or_else(|| sub_matches.get_one::<String>("pkey").unwrap())
                .as_str();
            let base_dir = sub_matches.get_one::<String>("basedir").unwrap().as_str();
            if let Some(filter) = sub_matches.get_one::<String>("filter") {
                replace_with_subpath(
                    old_pkey,
                    pkey_path,
                    old_archive,
                    archive_path,
                    base_dir,
                    filter.as_str(),
                    sub_matches.get_flag("strip"),
                )
            } else {
                replace(old_pkey, pkey_path, old_archive, archive_path, base_dir)
            }
        }
        Some(("remove", sub_matches)) => {
            if let Some(filter) = sub_matches.get_one::<String>("filter") {
                remove_with_subpath(
                    pkey_path,
                    archive_path,
                    sub_matches.get_one::<String>("basedir").unwrap().as_str(),
                    filter.as_str(),
                    sub_matches.get_flag("strip"),
                )
            } else {
                remove(
                    pkey_path,
                    archive_path,
                    sub_matches.get_one::<String>("basedir").unwrap().as_str(),
                )
            }
        }
        Some(("list", sub_matches)) => list(
            pkey_path,
            archive_path,
            sub_matches.get_one::<String>("filter").map(|s| s.as_str()),
            match sub_matches.get_flag("long") {
                true => Some("path,size,offset,mode"),
                false => None,
            },
        ),
        Some(("cat", sub_matches)) => cat(
            pkey_path,
            archive_path,
            sub_matches.get_one::<String>("entry").unwrap().as_str(),
        ),
        Some(("split", sub_matches)) => split(
            pkey_path,
            archive_path,
            sub_matches.get_one::<String>("head").unwrap().as_str(),
            sub_matches.get_one::<String>("data").map(|s| s.as_str()),
        ),
        Some(("verify", sub_matches)) => verify(
            pkey_path,
            archive_path,
            sub_matches.get_one::<String>("basedir").unwrap().as_str(),
        ),
        _ => Ok(()),
    }
}

fn main() {
    cli().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });
}
