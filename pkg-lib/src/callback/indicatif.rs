use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;

use crate::Callback;

#[derive(Clone)]
pub struct IndicatifCallback {
    pb: ProgressBar,
}

impl IndicatifCallback {
    pub fn new() -> Self {
        Self {
            pb: ProgressBar::hidden(),
        }
    }
}

impl Callback for IndicatifCallback {
    fn start_download(&mut self, length: u64, file: &str) {
        self.pb = ProgressBar::new(length);
        self.pb.set_style(ProgressStyle::with_template("{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));

        let msg = match Url::parse(file) {
            Err(_) => file.to_owned(),
            Ok(url) => url
                .path_segments()
                .and_then(|segments| segments.last())
                .unwrap_or(file).to_owned(),
        };

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
