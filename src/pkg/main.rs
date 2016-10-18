extern crate octavo;

use std::{env, str};
use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{self, Command};

fn create(package: &str) -> io::Result<()> {
    if ! Path::new(package).is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "package directory not found"));
    }

    Command::new("tar")
        .arg("cf")
        .arg(&format!("../{}.tar", package))
        .arg(".")
        .current_dir(package)
        .spawn()?
        .wait()?;

    Ok(())
}

fn download(package: &str) -> io::Result<String> {
    let tarfile = format!("{}.tar", package);
    if Path::new(&tarfile).is_file() {
        write!(stderr(), "* Already downloaded {}\n", package)?;
    } else {
        let host = "static.redox-os.org";
        let port = 80;
        let path = format!("pkg/{}.tar", package);

        write!(stderr(), "* Connecting to {}:{}\n", host, port)?;

        let mut stream = TcpStream::connect((host, port))?;

        write!(stderr(), "* Requesting {}\n", path)?;

        let request = format!("GET /{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host);
        stream.write(request.as_bytes())?;
        stream.flush()?;

        write!(stderr(), "* Waiting for response\n")?;

        let mut response = Vec::new();

        loop {
            let mut buf = [0; 65536];
            let count = stream.read(&mut buf)?;
            if count == 0 {
                break;
            }
            response.extend_from_slice(&buf[.. count]);
        }

        write!(stderr(), "* Received {} bytes\n", response.len())?;

        let mut header_end = 0;
        while header_end < response.len() {
            if response[header_end..].starts_with(b"\r\n\r\n") {
                break;
            }
            header_end += 1;
        }

        let mut status = (0, String::new());
        for line in unsafe { str::from_utf8_unchecked(&response[..header_end]) }.lines() {
            if line.starts_with("HTTP/1.1 ") {
                let mut args = line.split(' ').skip(1);
                if let Some(arg) = args.next() {
                    if let Ok(status_code) = arg.parse::<usize>() {
                        status.0 = status_code;
                    }
                }

                status.1 = args.collect::<Vec<&str>>().join(" ");
            }
            write!(stderr(), "> {}\n", line)?;
        }

        match status.0 {
            200 => {
                write!(stderr(), "* Success {} {}\n", status.0, status.1)?;

                File::create(&tarfile)?.write(&response[header_end + 4 ..])?;
            },
            _ => {
                write!(stderr(), "* Failure {} {}\n", status.0, status.1)?;
                return Err(io::Error::new(io::ErrorKind::NotFound, "package archive not found"));
            }
        }
    }

    Ok(tarfile)
}

fn install(package: &str) -> io::Result<()> {
    let tarfile = download(package)?;

    Command::new("tar")
        .arg("tf")
        .arg(&tarfile)
        .spawn()?
        .wait()?;

    Ok(())
}

fn list(package: &str) -> io::Result<()> {
    let tarfile = download(package)?;

    Command::new("tar")
        .arg("tf")
        .arg(&tarfile)
        .spawn()?
        .wait()?;

    Ok(())
}

fn sign(package: &str) -> io::Result<()> {
    use octavo::octavo_digest::Digest;
    use octavo::octavo_digest::sha3::Sha512;

    let tarfile = download(package)?;

    let mut data = vec![];
    File::open(&tarfile)?.read_to_end(&mut data)?;

    let mut output = vec![0; Sha512::output_bytes()];
    let mut hash = Sha512::default();
    hash.update(&data);
    hash.result(&mut output);

    let mut encoded = String::new();
    for b in output.iter() {
        encoded.push_str(&format!("{:X}", b));
    }

    println!("{}", encoded);

    Ok(())
}

fn usage() -> io::Result<()> {
    write!(io::stderr(), "pkg [command] [arguments]\n")?;
    write!(io::stderr(), "    create [directory] - create a package\n")?;
    write!(io::stderr(), "    help - show this help message\n")?;
    write!(io::stderr(), "    install [package] - install a package\n")?;
    write!(io::stderr(), "    list [package] - list package contents\n")?;
    write!(io::stderr(), "    sign [package] - get the package signature\n")?;

    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);

    if let Some(op) = args.next() {
        match op.as_str() {
            "create" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        if let Err(err) = create(package) {
                            let _ = write!(io::stderr(), "pkg: create: {}: failed: {}\n", package, err);
                        } else {
                            let _ = write!(io::stderr(), "pkg: create: {}: succeeded\n", package);
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: create: no packages specified\n");
                    process::exit(1);
                }
            },
            "help" => {
                let _ = usage();
            },
            "install" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        if let Err(err) = install(package) {
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
                        if let Err(err) = list(package) {
                            let _ = write!(io::stderr(), "pkg: list: {}: failed: {}\n", package, err);
                        } else {
                            let _ = write!(io::stderr(), "pkg: list: {}: succeeded\n", package);
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: list: no packages specified\n");
                    process::exit(1);
                }
            }
            "sign" => {
                let packages: Vec<String> = args.collect();
                if ! packages.is_empty() {
                    for package in packages.iter() {
                        if let Err(err) = sign(package) {
                            let _ = write!(io::stderr(), "pkg: sign: {}: failed: {}\n", package, err);
                        } else {
                            let _ = write!(io::stderr(), "pkg: sign: {}: succeeded\n", package);
                        }
                    }
                } else {
                    let _ = write!(io::stderr(), "pkg: sign: no packages specified\n");
                    process::exit(1);
                }
            }
            _ => {
                let _ = write!(io::stderr(), "pkg: {}: unknown operation\n", op);
                let _ = usage();
                process::exit(1);
            }
        }
    } else {
        let _ = write!(io::stderr(), "pkg: no operation\n");
        let _ = usage();
        process::exit(1);
    }
}
