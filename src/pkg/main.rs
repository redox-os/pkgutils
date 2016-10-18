extern crate octavo;

use std::{env, str};
use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::net::TcpStream;
use std::process::{self, Command};

fn install(package: &str) -> io::Result<()> {
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

            let path = format!("{}.tar", package);

            let mut file = File::create(&path)?;
            file.write(&response[header_end + 4 ..])?;

            Command::new("tar")
                .arg("tf")
                .arg(&path)
                .spawn()?
                .wait()?;
        },
        _ => {
            write!(stderr(), "* Failure {} {}\n", status.0, status.1)?;
        }
    }

    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);

    if let Some(op) = args.next() {
        match op.as_str() {
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
            _ => {
                let _ = write!(io::stderr(), "pkg: {}: unknown operation\n", op);
                process::exit(1);
            }
        }
    } else {
        let _ = write!(io::stderr(), "pkg: no operation\n");
        process::exit(1);
    }
}
