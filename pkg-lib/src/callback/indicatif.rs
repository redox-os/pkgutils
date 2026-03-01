use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;

use crate::{
    backend::Error,
    callback::{Callback, PlainCallback},
    package::RemotePackage,
    PackageList,
};

#[derive(Clone)]
pub struct IndicatifCallback {
    pb: ProgressBar,
    unknown_len: bool,
    fallback: PlainCallback,
    has_download: bool,
}

impl IndicatifCallback {
    pub fn new() -> Self {
        Self {
            pb: ProgressBar::hidden(),
            unknown_len: false,
            fallback: PlainCallback::new(),
            has_download: false,
        }
    }

    pub fn set_interactive(&mut self, enabled: bool) {
        self.fallback.set_interactive(enabled);
    }

    fn fetch_style(&self) -> ProgressStyle {
        ProgressStyle::with_template(
          "{prefix:>12.cyan.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})"
        )
        .unwrap()
        .progress_chars("=> ")
    }

    fn download_style(&self) -> ProgressStyle {
        ProgressStyle::with_template(
                "{prefix:>12.green.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            )
            .unwrap()
            .progress_chars("=> ")
    }

    fn commit_style(&self) -> ProgressStyle {
        ProgressStyle::with_template(
          "{prefix:>12.yellow.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})"
        )
        .unwrap()
        .progress_chars("=> ")
    }

    fn abort_style() -> ProgressStyle {
        ProgressStyle::with_template(
        "{prefix:>12.red.bold} {msg} [{elapsed_precise}] [{wide_bar:.red/blue}] {pos}/{len} ({eta})",
    )
    .unwrap()
    .progress_chars("=> ")
    }
}

impl Callback for IndicatifCallback {
    fn fetch_start(&mut self, initial_count: usize) {
        self.pb = ProgressBar::new(initial_count as u64);
        self.pb.set_style(self.fetch_style());
        self.pb.set_prefix("Fetching");
        self.pb.set_message("metadata");
    }

    fn fetch_package_name(&mut self, pkg_name: &crate::PackageName) {
        self.pb.set_message(pkg_name.to_string());
    }

    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize) {
        if added_count > 0 {
            self.pb.inc_length(added_count as u64);
        }
        if added_processed > 0 {
            self.pb.inc(added_processed as u64);
        }
    }

    fn fetch_end(&mut self) {
        self.pb.finish_and_clear();
        self.fallback.fetch_end();
    }

    fn install_prompt(&mut self, list: &PackageList) -> Result<(), Error> {
        self.pb.suspend(|| self.fallback.install_prompt(list))
    }

    fn install_extract(&mut self, remote_pkg: &RemotePackage) {
        self.pb
            .println(format!("Extracting {}", remote_pkg.package.name));
    }

    fn download_start(&mut self, length: u64, file: &str) {
        self.pb.suspend(|| {
            self.fallback.download_start(length, file);
        });

        self.pb = ProgressBar::new(length);
        self.unknown_len = length == 0;
        self.pb.set_style(self.download_style());
        self.pb.set_prefix("Downloading");

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

    fn download_increment(&mut self, downloaded: u64) {
        self.pb.inc(downloaded);
        if self.unknown_len {
            self.pb.inc_length(downloaded);
        }
    }

    fn download_end(&mut self) {
        self.pb.finish_and_clear();
        self.has_download = true;
    }

    fn commit_start(&mut self, count: usize) {
        if self.has_download {
            println!("Download complete.");
            self.has_download = false;
        }

        self.pb = ProgressBar::new(count as u64);
        self.unknown_len = count == 0;
        self.pb.set_style(self.commit_style());
        self.pb.set_prefix("Committing");
        self.pb.set_message("transaction changes");
    }

    fn commit_increment(&mut self, _file: &pkgar::Transaction) {
        self.pb.inc(1);
        if self.unknown_len {
            self.pb.inc_length(1);
        }
    }

    fn commit_end(&mut self) {
        let complete = self.pb.position() == self.pb.length().unwrap_or(0);
        self.pb.finish_and_clear();
        if complete {
            println!("Commit complete.");
        } else {
            println!("Commit incomplete.");
        }
    }

    fn abort_start(&mut self, count: usize) {
        self.pb = ProgressBar::new(count as u64);
        self.unknown_len = count == 0;
        self.pb.set_style(Self::abort_style());
        self.pb.set_prefix("Aborting");
        self.pb.set_message("reverting changes");
    }

    fn abort_increment(&mut self, _file: &pkgar::Transaction) {
        self.pb.inc(1);
        if self.unknown_len {
            self.pb.inc_length(1);
        }
    }

    fn abort_end(&mut self) {
        self.pb.finish_and_clear();
        println!("Transaction aborted successfully.");
    }
}
