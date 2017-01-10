use std::fs::File;
use std::io::{self, stderr, Read, Write};
use std::net::TcpStream;
use std::str::{self, Split};

fn parse_url<'a>(url: &'a str) -> (&'a str, u16, Split<'a, char>) {
    let mut parts = url.split('/');
    parts.next(); // Skip http://
    parts.next();
    let remote = parts.next().unwrap_or("");
    let mut remote_parts = remote.split(':');
    let host = remote_parts.next().unwrap_or("");
    let port = remote_parts.next().unwrap_or("").parse::<u16>().unwrap_or(80);
    (host, port, parts)
}

pub fn download(remote_path: &str, local_path: &str) -> io::Result<()> {
    let (host, port, parts) = parse_url(remote_path);

    let mut path = String::new();
    for part in parts {
        path.push('/');
        path.push_str(part);
    }

    write!(stderr(), "* Connecting to {}:{}\n", host, port)?;

    let mut stream = TcpStream::connect((host, port))?;

    write!(stderr(), "* Requesting {}\n", path)?;

    let request = format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host);
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

            File::create(&local_path)?.write(&response[header_end + 4 ..])?;
            Ok(())
        },
        _ => {
            write!(stderr(), "* Failure {} {}\n", status.0, status.1)?;

            Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", remote_path)))
        }
    }
}
