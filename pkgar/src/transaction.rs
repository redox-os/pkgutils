use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};

use blake3::Hash;
use pkgar_core::{Entry, Mode, PackageSrc};

use crate::ext::{EntryExt, PackageSrcExt, copy_and_hash, copy_and_hash2, diff_package};
use crate::{Error, READ_WRITE_HASH_BUF_SIZE, wrap_io_err};

/// Determine the temporary path for a file, and create its parent directories.
/// Returns `Err` if the target path is invalid or I/O error.
fn temp_path(target_path: impl AsRef<Path>, entry_hash: Hash) -> Result<PathBuf, Error> {
    let target_path = target_path.as_ref();
    let (Some(dir), Some(base)) = (target_path.parent(), target_path.file_name()) else {
        return Err(Error::InvalidPathComponent {
            invalid: target_path.into(),
            path: target_path.into(),
            entry: None,
        });
    };
    if !dir.is_dir() {
        fs::create_dir_all(dir).map_err(wrap_io_err!(dir.to_path_buf(), "Creating dir"))?;
    }

    let tmp_path = dir
        .join(".pkgar")
        .with_added_extension(base)
        .with_added_extension(entry_hash.to_hex().as_str());

    match std::fs::remove_file(&tmp_path) {
        Ok(_) => {}
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(wrap_io_err!(dir.to_path_buf(), "Removing old temp file")(e)),
    }
    Ok(tmp_path)
}

/// Individual atomic file operation
#[derive(Clone, Debug)]
pub enum Action {
    /// Temp files (`.pkgar.*`) to target files
    Rename(PathBuf, PathBuf),
    Remove(PathBuf),
}

impl Action {
    fn commit(&self) -> Result<(), Error> {
        match self {
            Action::Rename(tmp, target) => {
                fs::rename(tmp, target).map_err(wrap_io_err!(tmp.to_path_buf(), "Renaming file"))
            }
            Action::Remove(target) => {
                fs::remove_file(target).map_err(wrap_io_err!(target.to_path_buf(), "Removing file"))
            }
        }
    }

    fn abort(&self) -> Result<(), Error> {
        match self {
            Action::Rename(tmp, _) => {
                fs::remove_file(tmp).map_err(wrap_io_err!(tmp.to_path_buf(), "Removing tempfile"))
            }
            Action::Remove(_) => Ok(()),
        }
    }

    /// Returns the file path it's targeting into
    pub fn target_file(&self) -> &Path {
        match self {
            Action::Rename(_, path) => path.as_path(),
            Action::Remove(path) => path.as_path(),
        }
    }

    pub fn is_removal(&self) -> bool {
        matches!(self, Action::Remove(..))
    }
}

/// A struct that holds many atomic file operation
pub struct Transaction {
    actions: Vec<Action>,
    /// Map of relative path and (src, is_removal)
    path_map: BTreeMap<PathBuf, (Option<String>, bool)>,
    possible_conflicts: Vec<TransactionConflict>,
    ignored_entries: Vec<TransactionIgnored>,
    // this is here to avoid allocating too often
    buf: Box<[u8]>,
    indexed: usize,
    committed: usize,
    callback: Option<Box<dyn FnMut(u64, u64)>>,
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Transaction {
    /// Creates new empty transaction that can be added later
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            path_map: BTreeMap::new(),
            possible_conflicts: Vec::new(),
            ignored_entries: Vec::new(),
            committed: 0,
            indexed: 0,
            buf: vec![0; READ_WRITE_HASH_BUF_SIZE].into_boxed_slice(),
            callback: None,
        }
    }

    /// Set a callback for in-file progress, the callback args is (progress, total) in bytes of each entry.
    pub fn set_callback<F: FnMut(u64, u64) + 'static>(&mut self, cb: F) {
        self.callback = Some(Box::new(cb));
    }

    /// Creates new transactions to install from a pkgar file. Overwrites any existing file.
    pub fn install<Pkg, R>(src: &mut Pkg, base_dir: impl AsRef<Path>) -> Result<Self, Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let entries = src.read_entries()?;
        let mut transaction = Transaction::new();
        transaction.install_with_entries(src, &entries, base_dir, true)?;
        Ok(transaction)
    }

    /// Add transactions to install from a pkgar file with filtered or modified entries.
    ///
    /// To allow overwriting existing files, set `skip_local_check` to `true`.
    pub fn install_with_entries<Pkg, R>(
        &mut self,
        src: &mut Pkg,
        entries: &[Entry],
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        for entry in entries {
            self.install_one(src, entry, &base_dir, skip_local_check)?
        }

        Ok(())
    }

    /// Create a new action to install an [`Entry`] of `src` to `base_dir`.
    ///
    /// To allow overwriting existing files, set `skip_local_check` to `true`.
    pub fn install_one<Pkg, R>(
        &mut self,
        src: &mut Pkg,
        entry: &Entry,
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let relative_path = entry.check_path()?;
        let target_path = base_dir.as_ref().join(relative_path);
        self.indexed += 1;
        if !skip_local_check {
            let existing_meta = fs::symlink_metadata(&target_path);
            if existing_meta.is_ok_and(|s| s.is_file() || s.is_symlink()) {
                self.report_ignored(
                    relative_path,
                    Some(src.path().to_string()),
                    TransactionIgnoredReason::Exists,
                );
                return Ok(());
            }
        }
        let tmp_path = temp_path(&target_path, entry.blake3())?;
        let mode = entry.mode().map_err(Error::from)?;
        let mut data_reader = src.data_reader(entry)?;
        let data_size = Some(data_reader.data_size);
        let (entry_data_size, entry_data_hash) = match mode.kind() {
            Mode::FILE => {
                let mut tmp_file = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .mode(mode.perm().bits())
                    .open(&tmp_path)
                    .map_err(wrap_io_err!(tmp_path, "Opening tempfile"))?;

                self.copy_and_hash(&mut data_reader, &mut tmp_file, data_size)?
            }
            Mode::SYMLINK => {
                let mut data = Vec::new();
                let (size, hash) = self.copy_and_hash(&mut data_reader, &mut data, data_size)?;
                let sym_target = Path::new(OsStr::from_bytes(&data));
                symlink(sym_target, &tmp_path)
                    .map_err(wrap_io_err!(tmp_path, "Symlinking to tmp"))?;
                (size, hash)
            }
            _ => {
                return Err(Error::from(pkgar_core::Error::InvalidMode(mode.bits())));
            }
        };
        entry.verify(entry_data_hash, entry_data_size, &data_reader)?;
        data_reader.finish(src)?;
        self.push_action(Action::Rename(tmp_path, target_path), Some(src));
        Ok(())
    }

    /// Create new transactions to replace old files from a pkgar file.
    /// Does not overwrite existing file if the file is not updated between two package.
    /// Does not replace or remove existing file if the file is changed locally.
    pub fn replace<PkgOld, PkgNew, R1, R2>(
        old: &mut PkgOld,
        new: &mut PkgNew,
        base_dir: impl AsRef<Path>,
    ) -> Result<Transaction, Error>
    where
        R1: io::Read + io::Seek,
        R2: io::Read + io::Seek,
        PkgOld: PackageSrc<Err = Error> + PackageSrcExt<R1>,
        PkgNew: PackageSrc<Err = Error> + PackageSrcExt<R2>,
    {
        let old_entries = old.read_entries()?;
        let new_entries = new.read_entries()?;
        let mut transaction = Transaction::new();
        transaction.replace_with_entries(
            &old_entries,
            &new_entries,
            Some(old),
            new,
            base_dir,
            false,
        )?;
        Ok(transaction)
    }

    /// Add transactions to replace old files from a pkgar file with filtered or modified entries.
    /// To skip checking and allow overwrite locally modified files being replaced, set `skip_local_check` to true.
    pub fn replace_with_entries<PkgOld, PkgNew, R1, R2>(
        &mut self,
        old_entries: &[Entry],
        new_entries: &[Entry],
        old: Option<&PkgOld>,
        new: &mut PkgNew,
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R1: io::Read + io::Seek,
        R2: io::Read + io::Seek,
        PkgOld: PackageSrc<Err = Error> + PackageSrcExt<R1>,
        PkgNew: PackageSrc<Err = Error> + PackageSrcExt<R2>,
    {
        let (entries_to_install, entries_to_update, entries_to_remove) =
            self.replace_diff(old_entries, new_entries)?;

        self.install_with_entries(new, &entries_to_install, &base_dir, skip_local_check)?;

        for (old_entry, new_entry) in entries_to_update {
            self.replace_one(new, &old_entry, &new_entry, &base_dir, skip_local_check)?;
        }

        self.remove_with_entries(old, &entries_to_remove, &base_dir, skip_local_check)?;

        Ok(())
    }

    /// Get diff between two packages, return tuple of two [`Vec`] that need to be
    /// consumed to [`Self::install_one`] and [`Self::remove_one`] (in that order).
    pub fn replace_diff(
        &mut self,
        old_entries: &[Entry],
        new_entries: &[Entry],
    ) -> Result<(Vec<Entry>, Vec<(Entry, Entry)>, Vec<Entry>), Error> {
        let (entries_to_install, entries_to_update, entries_to_remove, skipped_entries) =
            diff_package(old_entries, new_entries)?;

        // neither added nor removed, so count as two
        self.indexed += 2 * skipped_entries;

        Ok((entries_to_install, entries_to_update, entries_to_remove))
    }

    /// Create a new action to install an [`Entry`] to `base_dir` with check against old entry.
    /// The entry path against two entry must be identical, otherwise don't use this function.
    ///
    /// Setting `skip_local_check` to `true` is equivalent to install_one with the same option.
    pub fn replace_one<Pkg, R>(
        &mut self,
        src: &mut Pkg,
        old_entry: &Entry,
        new_entry: &Entry,
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let relative_path = old_entry.check_path()?;
        let target_path = base_dir.as_ref().join(relative_path);
        self.indexed += 1;

        // Generally user don't care if this not exist, even without `skip_local_check`
        let mode = match fs::symlink_metadata(&target_path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(e) => {
                return Err(wrap_io_err!(
                    target_path.clone(),
                    "Opening file metadata for removal"
                )(e));
            }
        };

        if skip_local_check {
            self.install_one(src, new_entry, base_dir, true)?;
            return Ok(());
        }

        let (_, entry_data_hash) = match (mode.is_file(), mode.is_symlink()) {
            (true, false) => {
                let mut existing = File::open(&target_path)
                    .map_err(wrap_io_err!(&target_path, "Opening file for removal"))?;

                self.copy_and_hash(&mut existing, &mut io::sink(), None)?
            }
            (_, true) => {
                let existing = fs::read_link(&target_path)
                    .map_err(wrap_io_err!(&target_path, "Opening symlink for removal"))?;
                let mut data_reader = existing.as_os_str().as_bytes();
                self.copy_and_hash(&mut data_reader, &mut io::sink(), None)?
            }
            _ => {
                return Err(Error::from(pkgar_core::Error::InvalidMode(mode.mode())));
            }
        };

        if entry_data_hash == old_entry.blake3() {
            self.install_one(src, new_entry, base_dir, true)?;
        } else {
            self.report_ignored(
                relative_path,
                Some(src.path().to_string()),
                TransactionIgnoredReason::Modified,
            );
        }
        Ok(())
    }

    /// Prepare transactions to remove files from a pkgar file.  Does not remove files with different hash
    pub fn remove<Pkg, R>(src: &mut Pkg, base_dir: impl AsRef<Path>) -> Result<Self, Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let entries = src.read_entries()?;
        let mut transaction = Transaction::new();
        transaction.remove_with_entries(Some(src), &entries, base_dir, false)?;
        Ok(transaction)
    }

    /// Prepare transactions to remove files from a pkgar file with filtered or modified entries.
    ///
    /// To skip checking and allow removal for locally modified files, set `skip_local_check` to `true`.
    pub fn remove_with_entries<Pkg, R>(
        &mut self,
        src: Option<&Pkg>,
        entries: &[Entry],
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        for entry in entries {
            self.remove_one(src, entry, &base_dir, skip_local_check)?;
        }
        Ok(())
    }

    /// Create a new action to uninstall an [`Entry`] to `base_dir`.
    ///
    /// To skip checking and allow removal for locally modified files, set `skip_local_check` to `true`.
    pub fn remove_one<Pkg, R>(
        &mut self,
        src: Option<&Pkg>,
        entry: &Entry,
        base_dir: impl AsRef<Path>,
        skip_local_check: bool,
    ) -> Result<(), Error>
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let relative_path = entry.check_path()?;
        let target_path = base_dir.as_ref().join(relative_path);
        self.indexed += 1;

        if skip_local_check {
            self.push_action(Action::Remove(target_path), src);
            return Ok(());
        }

        // Generally user don't care if this not exist, even without `skip_local_check`
        let mode = match fs::symlink_metadata(&target_path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !skip_local_check {
                    self.report_ignored(
                        relative_path,
                        src.map(|s| s.path().to_string()),
                        TransactionIgnoredReason::Missing,
                    );
                }
                return Ok(());
            }
            Err(e) => {
                return Err(wrap_io_err!(
                    target_path.clone(),
                    "Opening file metadata for removal"
                )(e));
            }
        };

        let (_, entry_data_hash) = match (mode.is_file(), mode.is_symlink()) {
            (true, false) => {
                let mut existing = File::open(&target_path)
                    .map_err(wrap_io_err!(&target_path, "Opening file for removal"))?;

                self.copy_and_hash(&mut existing, &mut io::sink(), None)?
            }
            (_, true) => {
                let existing = fs::read_link(&target_path)
                    .map_err(wrap_io_err!(&target_path, "Opening symlink for removal"))?;
                let mut data_reader = existing.as_os_str().as_bytes();
                self.copy_and_hash(&mut data_reader, &mut io::sink(), None)?
            }
            _ => {
                return Err(Error::from(pkgar_core::Error::InvalidMode(mode.mode())));
            }
        };

        if entry_data_hash == entry.blake3() {
            self.push_action(Action::Remove(target_path), src);
        } else {
            self.report_ignored(
                relative_path,
                src.map(|s| s.path().to_string()),
                TransactionIgnoredReason::Modified,
            );
        }
        Ok(())
    }

    /// Internal function to push new action
    fn push_action<Pkg, R>(&mut self, action: Action, src: Option<&Pkg>)
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        let action_key = action.target_file();
        match self.path_map.entry(action_key.to_path_buf()) {
            std::collections::btree_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert((src.map(|s| s.path().to_string()), action.is_removal()));
                self.actions.push(action);
            }
            std::collections::btree_map::Entry::Occupied(occupied_entry) => {
                match (occupied_entry.get().1, action.is_removal()) {
                    // safe to merge
                    (true, true) => {}
                    // When conflicts happened, it will not get overwritten
                    (false, false) => {
                        self.possible_conflicts.push(TransactionConflict {
                            conflicted_path: action_key.to_path_buf(),
                            former_src: occupied_entry.get().0.clone(),
                            newer_src: src.map(|s| s.path().to_string()),
                        });
                    }
                    // worst case just happened
                    (false, true) | (true, false) => {
                        let src = if !action.is_removal() {
                            occupied_entry.get().0.as_ref().map(|s| s.to_string())
                        } else {
                            src.map(|s| s.path().to_string())
                        };
                        self.report_ignored(action_key, src, TransactionIgnoredReason::Exists);
                        if !action.is_removal() {
                            // prioritize non-removal action to survive from removal,
                            // as we cannot abort this Action on this function
                            self.actions.insert(0, action);
                        }
                    }
                }
            }
        }
    }

    fn report_ignored(
        &mut self,
        path: &Path,
        src: Option<String>,
        reason: TransactionIgnoredReason,
    ) {
        self.ignored_entries.push(TransactionIgnored {
            ignored_path: path.to_path_buf(),
            src,
            reason,
        });
    }
    /// Same as [`copy_and_hash`]
    fn copy_and_hash<R: std::io::Read, W: std::io::Write>(
        &mut self,
        read: &mut R,
        write: &mut W,
        total_size: Option<u64>,
    ) -> Result<(u64, Hash), Error> {
        let mut cb = self.callback.take();

        let res = if let (Some(callback_fn), Some(total_size)) = (cb.as_mut(), total_size) {
            copy_and_hash2(
                read,
                write,
                &mut self.buf,
                Some(&mut |bytes| {
                    callback_fn(bytes, total_size);
                }),
            )
        } else {
            copy_and_hash(read, write, &mut self.buf)
        };

        self.callback = cb;

        res.map_err(wrap_io_err!("Copying and hashing file"))
    }

    /// Apply all pending actions from end to start.
    /// This resets the committed counter back to zero.
    /// if failed abort() is needed to clean up pending transaction.
    pub fn commit(&mut self) -> Result<usize, Error> {
        self.reset_committed();
        while !self.actions.is_empty() {
            self.commit_one()?;
        }
        Ok(self.committed)
    }

    /// Apply one last item from actions stack,
    /// returns how many transactions committed since last counter reset.
    pub fn commit_one(&mut self) -> Result<usize, Error> {
        if let Some(action) = self.actions.pop() {
            if let Err(err) = action.commit() {
                // Should be possible to restart a failed transaction
                self.actions.push(action);
                return Err(Error::FailedCommit {
                    source: Box::new(err),
                    changed: self.committed,
                    remaining: self.actions.len(),
                });
            }
            self.committed += 1;
        }
        Ok(self.committed)
    }

    /// Clean up any tmp files referenced by this transaction without committing.
    /// Note that this function will check all actions and only after it has attempted
    /// to abort them all will it return an error with context info. Remaining actions
    /// are left as a part of this transaction to allow for re-runs of this function.
    pub fn abort(&mut self) -> Result<usize, Error> {
        let mut last_failed = false;
        self.reset_committed();
        while !self.actions.is_empty() {
            if let Err(err) = self.abort_one() {
                if last_failed {
                    return Err(err);
                } else {
                    last_failed = true;
                }
            }
        }
        Ok(self.committed)
    }

    /// Abort one last item from actions stack
    pub fn abort_one(&mut self) -> Result<usize, Error> {
        if let Some(action) = self.actions.pop() {
            if let Err(err) = action.abort() {
                // This is inherently inefficent, no biggie
                self.actions.insert(0, action);
                return Err(Error::FailedCommit {
                    source: Box::new(err),
                    changed: self.committed,
                    remaining: self.actions.len(),
                });
            }
            self.committed += 1;
        }
        Ok(self.committed)
    }

    /// Get how much actions are pending to commit.
    pub fn pending_commit(&self) -> usize {
        self.actions.len()
    }

    /// Get how much actions committed.
    /// Aborted actions also counts.
    pub fn total_committed(&self) -> usize {
        self.committed
    }

    /// Resets committed counter
    pub fn reset_committed(&mut self) {
        self.committed = 0;
    }

    /// Get how much entry is indexed.
    /// Any entry that have been added is counted as indexed no matter it will be processed or not.
    pub fn total_indexed(&self) -> usize {
        self.indexed
    }

    /// Resets indexed counter
    pub fn reset_indexed(&mut self) {
        self.indexed = 0;
    }

    /// Peek pending actions.
    /// Actions are executed from last item.
    pub fn get_actions(&self) -> &[Action] {
        &self.actions
    }

    /// Get list of conflicted actions and their sources if given.
    /// The action that is actually used will be the newer one.
    pub fn get_possible_conflicts(&self) -> &[TransactionConflict] {
        &self.possible_conflicts
    }

    /// Get list of entries that is ignored
    pub fn get_ignored_entries(&self) -> &[TransactionIgnored] {
        &self.ignored_entries
    }

    /// Add a newer transaction with their source package for optional conflict identification
    pub fn merge<Pkg, R>(&mut self, newer: Transaction, src: Option<&Pkg>)
    where
        R: io::Read + io::Seek,
        Pkg: PackageSrc<Err = Error> + PackageSrcExt<R>,
    {
        self.indexed += newer.indexed;
        self.committed += newer.committed;
        for action in newer.actions {
            self.push_action(action, src);
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionConflict {
    pub conflicted_path: PathBuf,
    pub former_src: Option<String>,
    pub newer_src: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransactionIgnored {
    pub ignored_path: PathBuf,
    pub src: Option<String>,
    pub reason: TransactionIgnoredReason,
}

#[derive(Debug, Clone, Copy)]
pub enum TransactionIgnoredReason {
    /// The file is already gone
    Missing,
    /// The file have different hash
    Modified,
    /// The file is already exist
    Exists,
}
