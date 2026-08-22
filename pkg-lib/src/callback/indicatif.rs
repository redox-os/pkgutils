use indicatif::{ProgressBar, ProgressStyle};

use crate::callback::{Callback, PlainCallback};
#[cfg(feature = "library")]
use crate::{backend::Error, package::RemotePackage, PackageList};

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

    pub fn set_always_yes(&mut self, enabled: Option<bool>) {
        self.fallback.set_always_yes(enabled);
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

    #[cfg(feature = "library")]
    fn extract_style(&self) -> ProgressStyle {
        ProgressStyle::with_template(
                "{prefix:>12.yellow.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("=> ")
    }

    #[cfg(feature = "library")]
    fn commit_style(&self) -> ProgressStyle {
        ProgressStyle::with_template(
          "{prefix:>12.orange.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})"
        )
        .unwrap()
        .progress_chars("=> ")
    }

    #[cfg(feature = "library")]
    fn abort_style() -> ProgressStyle {
        ProgressStyle::with_template(
        "{prefix:>12.red.bold} {msg} [{percent:>3}%] [{elapsed_precise}] [{wide_bar:.red/blue}] {pos}/{len} ({eta})",
    )
    .unwrap()
    .progress_chars("=> ")
    }
}

impl Callback for IndicatifCallback {
    fn fetch_start(&mut self, initial_count: usize) {
        self.pb = ProgressBar::new(initial_count as u64);
        self.pb.set_style(self.fetch_style());
        self.pb.set_prefix(self.fallback.fetching_str());
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

    #[cfg(feature = "library")]
    fn install_prompt(&mut self, list: &PackageList) -> Result<(), Error> {
        self.pb.suspend(|| self.fallback.install_prompt(list))
    }

    #[cfg(feature = "library")]
    fn install_check(
        &mut self,
        conflict: &[pkgar::TransactionConflict],
        ignored: &[pkgar::TransactionIgnored],
    ) -> Result<(), Error> {
        self.pb
            .suspend(|| self.fallback.install_check(conflict, ignored))
    }

    fn download_start(&mut self, length: u64, file: &str) {
        self.unknown_len = length == 0;
        if self.unknown_len {
            // to not interrupt the fetch progress bar
            return;
        }
        self.pb = ProgressBar::new(length);
        self.pb.set_style(self.download_style());
        self.pb.set_prefix(self.fallback.downloading_str());

        let msg = match file.rsplit_once('/') {
            Some((_, file)) => file,
            None => file,
        }
        .to_owned();

        self.pb.set_message(msg);
    }

    fn download_increment(&mut self, downloaded: u64) {
        if self.unknown_len {
            return;
        }
        self.pb.inc(downloaded);
    }

    fn download_end(&mut self) {
        if self.unknown_len {
            return;
        }
        self.pb.finish_and_clear();
        self.has_download = true;
    }

    #[cfg(feature = "library")]
    fn extract_start(&mut self, pkg_name: &RemotePackage, index_count: usize) {
        self.unknown_len = index_count == 0;
        self.pb = ProgressBar::new(index_count as u64);
        self.pb.set_style(self.extract_style());
        self.pb.set_prefix(self.fallback.extracting_str());
        self.pb.set_message(pkg_name.package.name.to_string());
    }

    #[cfg(feature = "library")]
    fn extract_increment(&mut self, indexed: usize) {
        self.pb.inc(indexed as u64);
        if self.unknown_len {
            self.pb.inc_length(indexed as u64);
        }
    }

    #[cfg(feature = "library")]
    fn extract_end(&mut self) {
        self.pb.finish_and_clear();
    }

    #[cfg(feature = "library")]
    fn uncheck_start(&mut self, pkg_name: &crate::PackageName, index_count: usize) {
        self.unknown_len = index_count == 0;
        self.pb = ProgressBar::new(index_count as u64);
        self.pb.set_style(self.extract_style());
        self.pb.set_prefix(self.fallback.checking_str());
        self.pb.set_message(pkg_name.to_string());
    }

    #[cfg(feature = "library")]
    fn uncheck_increment(&mut self, indexed: usize) {
        self.pb.inc(indexed as u64);
        if self.unknown_len {
            self.pb.inc_length(indexed as u64);
        }
    }

    #[cfg(feature = "library")]
    fn uncheck_end(&mut self) {
        self.pb.finish_and_clear();
    }

    #[cfg(feature = "library")]
    fn commit_start(&mut self, count: usize) {
        if self.has_download {
            println!("Download complete.");
            self.has_download = false;
        }

        self.pb = ProgressBar::new(count as u64);
        self.unknown_len = count == 0;
        self.pb.set_style(self.commit_style());
        self.pb.set_prefix(self.fallback.committing_str());
        self.pb.set_message("changes");
    }

    #[cfg(feature = "library")]
    fn commit_increment(&mut self, _file: &pkgar::Transaction) {
        self.pb.inc(1);
        if self.unknown_len {
            self.pb.inc_length(1);
        }
    }

    #[cfg(feature = "library")]
    fn commit_end(&mut self) {
        let complete = self.pb.position() == self.pb.length().unwrap_or(0);
        self.pb.finish_and_clear();
        if complete {
            println!("Commit complete.");
        } else {
            println!("Commit incomplete.");
        }
    }

    #[cfg(feature = "library")]
    fn abort_start(&mut self, count: usize) {
        self.pb = ProgressBar::new(count as u64);
        self.unknown_len = count == 0;
        self.pb.set_style(Self::abort_style());
        self.pb.set_prefix(self.fallback.aborting_str());
        self.pb.set_message("changes");
    }

    #[cfg(feature = "library")]
    fn abort_increment(&mut self, _file: &pkgar::Transaction) {
        self.pb.inc(1);
        if self.unknown_len {
            self.pb.inc_length(1);
        }
    }

    #[cfg(feature = "library")]
    fn abort_end(&mut self) {
        self.pb.finish_and_clear();
        if !self.unknown_len {
            println!("Transaction aborted successfully.");
        }
    }
}
