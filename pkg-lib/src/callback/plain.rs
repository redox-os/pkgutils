use std::{io::Write, time::Instant};

#[cfg(feature = "library")]
use crate::backend::Error;
use crate::{callback::Callback, package::RemotePackage};

#[derive(Clone)]
pub struct PlainCallback {
    unknown_size: bool,
    pos: u64,
    size: u64,
    file: Option<String>,

    interactive: bool,
    last_updated: Instant,
}

impl PlainCallback {
    pub fn new() -> Self {
        Self {
            size: 0,
            unknown_size: false,
            pos: 0,
            interactive: false,
            file: None,
            last_updated: Instant::now(),
        }
    }

    /// Set if user require to agree on terminal
    pub fn set_interactive(&mut self, enabled: bool) {
        self.interactive = enabled;
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }

    /// Convert bytes into readable string
    pub fn format_bytes(len: u64) -> String {
        const GB: u64 = 1024 * 1024 * 1024;
        const MB: u64 = 1024 * 1024;
        const KB: u64 = 1024;

        if len > GB {
            Self::format_bytes_inner(len, GB, "GB")
        } else if len > MB {
            Self::format_bytes_inner(len, MB, "MB")
        } else if len > KB {
            Self::format_bytes_inner(len, KB, "KB")
        } else {
            format!("{len} B")
        }
    }

    fn format_bytes_inner(len: u64, divisor: u64, suffix: &'static str) -> String {
        use std::fmt::Write;
        let mut s = format!("{}", len / divisor);
        if s.len() == 1 {
            let _ = write!(s, ".{:02}", (len % divisor) / (divisor / 100));
        } else if s.len() == 2 {
            let _ = write!(s, ".{:01}", (len % divisor) / (divisor / 10));
        }

        let _ = write!(s, " {suffix}");
        s
    }

    fn should_update_progress<F>(&mut self, do_print: F, force: bool)
    where
        F: Fn(&PlainCallback),
    {
        let now = Instant::now();
        // 20 FPS, same default with indicatif
        if force || now.duration_since(self.last_updated).as_millis() >= 50 {
            self.last_updated = now;
            do_print(&self);
        }
    }

    #[cfg(feature = "library")]
    fn confirm_transaction(&self) -> Result<(), Error> {
        if self.interactive {
            eprint!("\nProceed with this transaction? [Y/n]: ");
            self.flush();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or(0);
            let input = input.trim().to_lowercase();

            if input == "n" || input == "no" {
                return Err(Error::Interrupted);
            }
        } else {
            eprintln!();
        }

        Ok(())
    }

    // These maybe localized in future
    pub(crate) const fn fetching_str(&self) -> &'static str {
        "Fetching"
    }
    pub(crate) const fn downloading_str(&self) -> &'static str {
        "Downloading"
    }
    pub(crate) const fn extracting_str(&self) -> &'static str {
        "Extracting"
    }
    pub(crate) const fn checking_str(&self) -> &'static str {
        "Checking"
    }
    pub(crate) const fn committing_str(&self) -> &'static str {
        "Committing"
    }
    pub(crate) const fn aborting_str(&self) -> &'static str {
        "Aborting"
    }
}

const RESET_LINE: &str = "\r\x1b[2K";

impl Callback for PlainCallback {
    fn fetch_start(&mut self, initial_count: usize) {
        self.size = 0;
        self.pos = 0;
        self.fetch_package_increment(0, initial_count);
    }

    fn fetch_package_name(&mut self, pkg_name: &crate::PackageName) {
        // resuming after fetch_package_increment
        eprint!(" {}", pkg_name.as_str());
        self.flush();
    }

    fn fetch_package_increment(&mut self, added_processed: usize, added_count: usize) {
        self.pos += added_processed as u64;
        self.size += added_count as u64;

        self.should_update_progress(
            |this| {
                eprint!(
                    "{RESET_LINE}{}: [{}/{}]",
                    this.fetching_str(),
                    this.pos,
                    this.size
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    fn fetch_end(&mut self) {
        if self.pos == self.size {
            eprintln!("{RESET_LINE}Fetch complete.");
        } else {
            eprintln!("{RESET_LINE}Fetch incomplete.");
        }
    }

    #[cfg(feature = "library")]
    fn install_prompt(&mut self, list: &crate::PackageList) -> Result<(), Error> {
        eprintln!("");
        if !list.install.is_empty() {
            eprintln!("Packages to install:");
            for pkg in &list.install {
                eprintln!("  + {}", pkg);
            }
        }

        if !list.update.is_empty() {
            eprintln!("Packages to update:");
            for pkg in &list.update {
                eprintln!("  ~ {}", pkg);
            }
        }

        if !list.uninstall.is_empty() {
            eprintln!("Packages to uninstall:");
            for pkg in &list.uninstall {
                eprintln!("  - {}", pkg);
            }
        }

        eprintln!();
        if list.network_size > 0 {
            eprintln!(
                "  Download size:  {}",
                Self::format_bytes(list.network_size)
            );
        }
        if list.install_size > 0 {
            eprintln!(
                "  Install size:   {}",
                Self::format_bytes(list.install_size)
            );
        }
        if list.uninstall_size > 0 {
            eprintln!(
                "  Uninstall size: {}",
                Self::format_bytes(list.uninstall_size)
            );
        }

        self.confirm_transaction()
    }

    #[cfg(feature = "library")]
    fn install_check_conflict(&mut self, list: &[pkgar::TransactionConflict]) -> Result<(), Error> {
        if list.is_empty() {
            return Ok(());
        }

        eprintln!("Transaction conflict detected:");
        for pkg in list {
            eprintln!(
                "  -> {} (from {:?} replaced by {:?})",
                pkg.conflicted_path.display(),
                pkg.former_src.as_ref().map(|p| p.as_str()).unwrap_or("?"),
                pkg.newer_src.as_ref().map(|p| p.as_str()).unwrap_or("?"),
            );
        }

        self.confirm_transaction()
    }

    fn download_start(&mut self, length: u64, file: &str) {
        self.size = length;
        self.unknown_size = length == 0;
        self.pos = 0;
        if !self.unknown_size {
            eprint!("{RESET_LINE}{} {file}", self.downloading_str());
            self.file = Some(file.to_string());
            self.flush();
        }
    }

    fn download_increment(&mut self, downloaded: u64) {
        self.pos += downloaded;
        if self.unknown_size {
            self.size += downloaded;
        }
        if self.unknown_size {
            return;
        }

        self.should_update_progress(
            |this| {
                // keep using MB for consistency
                let pos_mb = this.pos as f64 / 1_048_576.0;
                let size_mb = this.size as f64 / 1_048_576.0;
                let file_name = this.file.as_ref().map(|s| s.as_str()).unwrap_or("");

                eprint!(
                    "{RESET_LINE}{} {} [{:.2} MB / {:.2} MB]",
                    this.downloading_str(),
                    file_name,
                    pos_mb,
                    size_mb
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    fn download_end(&mut self) {
        if !self.unknown_size {
            eprintln!("");
            self.file = None;
        }
    }

    #[cfg(feature = "library")]
    fn extract_start(&mut self, pkg_name: &RemotePackage, index_count: usize) {
        self.unknown_size = index_count == 0;
        self.size = index_count as u64;
        let file = &pkg_name.package.name;
        eprint!("{RESET_LINE}{} {file}", self.extracting_str());
        self.file = Some(file.to_string());
        self.flush();
    }

    #[cfg(feature = "library")]
    fn extract_increment(&mut self, indexed: usize) {
        self.pos += indexed as u64;
        if self.unknown_size {
            self.size += indexed as u64;
        }
        if self.unknown_size {
            return;
        }

        self.should_update_progress(
            |this| {
                let file_name = this.file.as_ref().map(|s| s.as_str()).unwrap_or("");
                eprint!(
                    "{RESET_LINE}{} {} [{}/{}]",
                    this.extracting_str(),
                    file_name,
                    this.pos,
                    this.size
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    #[cfg(feature = "library")]
    fn extract_end(&mut self) {
        if !self.unknown_size {
            eprintln!("");
            self.file = None;
        }
    }

    #[cfg(feature = "library")]
    fn uncheck_start(&mut self, pkg_name: &crate::PackageName, index_count: usize) {
        self.unknown_size = index_count == 0;
        self.size = index_count as u64;
        let file = pkg_name.as_str();
        eprintln!("{} {}...", self.extracting_str(), file);
        self.file = Some(file.to_string());
        self.flush();
    }

    #[cfg(feature = "library")]
    fn uncheck_increment(&mut self, indexed: usize) {
        self.pos += indexed as u64;
        if self.unknown_size {
            self.size += indexed as u64;
        }
        if self.unknown_size {
            return;
        }

        self.should_update_progress(
            |this| {
                let file_name = this.file.as_ref().map(|s| s.as_str()).unwrap_or("");
                eprint!(
                    "{RESET_LINE}{} {} [{}/{}]",
                    this.checking_str(),
                    file_name,
                    this.pos,
                    this.size
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    #[cfg(feature = "library")]
    fn uncheck_end(&mut self) {
        if !self.unknown_size {
            eprintln!("");
            self.file = None;
        }
    }

    #[cfg(feature = "library")]
    fn commit_start(&mut self, count: usize) {
        eprintln!("Committing changes...");
        self.size = count as u64;
        self.unknown_size = false;
        self.pos = 0;
        self.flush();
    }

    #[cfg(feature = "library")]
    fn commit_increment(&mut self, _file: &pkgar::Transaction) {
        self.pos += 1;
        if self.unknown_size {
            self.size += 1;
        }

        self.should_update_progress(
            |this| {
                eprint!(
                    "{RESET_LINE}{}: [{}/{}]",
                    this.committing_str(),
                    this.pos,
                    this.size
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    #[cfg(feature = "library")]
    fn commit_end(&mut self) {
        eprintln!("\nCommit done.");
    }

    #[cfg(feature = "library")]
    fn abort_start(&mut self, count: usize) {
        eprintln!("Aborting transaction...");
        self.size = count as u64;
        self.unknown_size = false;
        self.pos = 0;
        self.flush();
    }

    #[cfg(feature = "library")]
    fn abort_increment(&mut self, _file: &pkgar::Transaction) {
        self.pos += 1;
        if self.unknown_size {
            self.size += 1;
        }

        self.should_update_progress(
            |this| {
                eprint!(
                    "{RESET_LINE}{}: [{}/{}]",
                    this.aborting_str(),
                    this.pos,
                    this.size
                );
                this.flush();
            },
            self.pos == self.size,
        );
    }

    #[cfg(feature = "library")]
    fn abort_end(&mut self) {
        eprintln!("\nAbort done.");
    }
}
