use indicatif::{ProgressBar, ProgressStyle};

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
