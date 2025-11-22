use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;

use crate::Callback;

#[derive(Clone)]
pub struct IndicatifCallback {
    pb: ProgressBar,
    unknown_len: bool,
}

impl IndicatifCallback {
    pub fn new() -> Self {
        Self {
            pb: ProgressBar::hidden(),
            unknown_len: false,
        }
    }
}

impl Callback for IndicatifCallback {
    fn start_download(&mut self, length: u64, file: &str) {
        self.pb = ProgressBar::new(length);
        self.unknown_len = length == 0;
        self.pb.set_style(ProgressStyle::with_template("{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));

        let msg = match Url::parse(file) {
            Err(_) => file.to_owned(),
            Ok(url) => url
                .path_segments()
                .and_then(|segments| segments.last())
                .unwrap_or(file)
                .to_owned(),
        };

        self.pb.set_message(msg);
    }

    fn increment_downloaded(&mut self, downloaded: u64) {
        self.pb.inc(downloaded);
        if self.unknown_len {
            self.pb.inc_length(downloaded);
        }
    }
    fn end_download(&mut self) {
        self.pb.finish();
        println!();
    }
}
