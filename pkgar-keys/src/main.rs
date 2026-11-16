use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;

use clap::{Arg, ArgAction, Command, value_parser};
use clap::{crate_authors, crate_description, crate_name, crate_version};

use pkgar_keys::{
    DEFAULT_PUBKEY, DEFAULT_SECKEY, Error, SecretKeyFile, gen_keypair, get_skey, re_encrypt,
};

fn cli() -> Result<i32, Error> {
    let skey = Arg::new("skey")
        .short('s')
        .long("skey")
        .value_name("FILE")
        .help("Alternate secret keyfile")
        .global(true)
        .value_parser(value_parser!(PathBuf))
        .default_value_os(DEFAULT_SECKEY.as_os_str());

    let pkey = Arg::new("pkey")
        .short('p')
        .long("pkey")
        .value_name("FILE")
        .help("Alternate public keyfile")
        .value_parser(value_parser!(PathBuf))
        .default_value_os(DEFAULT_PUBKEY.as_os_str());

    let r#gen = Command::new("gen")
        .about("Generate a keypair and store on the filesystem")
        .arg(pkey)
        .arg(
            Arg::new("plaintext")
                .short('P')
                .long("plaintext")
                .help("Do not prompt for a passphrase and store the secret key as plain text")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help("Don't check for existing files before generating a new keypair")
                .action(ArgAction::SetTrue),
        );
    let reencrypt = Command::new("rencrypt").about("Re-encrypt the secret key provided by --skey");
    let export = Command::new("export")
        .about("Print the public key from --skey in the pkgar pubkey format")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .help("Output to a file instead of stdout"),
        );

    let matches = Command::new(crate_name!())
        .author(crate_authors!(", "))
        .about(crate_description!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(skey)
        .subcommand(r#gen)
        .subcommand(reencrypt)
        .subcommand(export)
        .get_matches();

    let skey_path = matches.get_one::<PathBuf>("skey").unwrap();

    match matches.subcommand() {
        Some(("gen", submatches)) => {
            if let Some(keydir) = skey_path.parent() {
                fs::create_dir_all(keydir).map_err(|source| Error::Io {
                    source,
                    path: Some(keydir.to_path_buf()),
                    context: "Creating directory",
                })?;
            }

            if !submatches.get_flag("force") && skey_path.exists() {
                return Err(Error::Io {
                    source: std::io::Error::from(std::io::ErrorKind::AlreadyExists),
                    path: Some(skey_path.clone()),
                    context: "Key already exist",
                });
            }

            let pkey_path = submatches.get_one::<PathBuf>("pkey").unwrap();

            if !submatches.get_flag("plaintext") {
                gen_keypair(pkey_path, skey_path)?;
            } else {
                let (pkey, skey) = SecretKeyFile::new();
                pkey.save(pkey_path)?;
                skey.save(skey_path)?;
            }
        }
        Some(("export", submatches)) => {
            let skey = get_skey(skey_path)?;
            let pkey = skey
                .public_key_file()
                .expect("Secret key was encrypted after being decrypted");

            if let Some(file) = submatches.get_one::<PathBuf>("file") {
                pkey.save(file)?;
            } else {
                pkey.write(io::stdout().lock())?;
            }
        }
        Some(("rencrypt", _)) => {
            re_encrypt(skey_path)?;
            println!("Successfully re-encrypted {}", skey_path.display());
        }
        _ => unreachable!(),
    }

    Ok(0)
}

fn main() {
    let code = cli().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        process::exit(1);
    });
    process::exit(code);
}
