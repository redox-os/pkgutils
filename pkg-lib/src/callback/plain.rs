use std::io::Write;

use crate::Callback;

#[derive(Clone)]
pub struct PlainCallback {
    size: u64,
    pos: u64,
    len: u64,
    seek: u64,
}

impl PlainCallback {
    pub fn new() -> Self {
        Self {
            size: 0,
            pos: 0,
            len: 10,
            seek: 0,
        }
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

impl Callback for PlainCallback {
    fn start_download(&mut self, length: u64, file: &str) {
        eprint!("Downloading {}", file);
        self.size = length;
        self.pos = 0;
        self.seek = 0;
        self.flush();
    }

    fn increment_downloaded(&mut self, downloaded: usize) {
        self.pos += downloaded as u64;
        let new_seek = (self.pos * self.len) / self.size;
        while self.seek < new_seek {
            self.seek += 1;
            eprint!(".");
        }
        self.flush();
    }

    fn end_download(&mut self) {
        while self.seek < self.len {
            self.seek += 1;
            eprint!(".");
        }
        eprintln!("done");
    }
}
