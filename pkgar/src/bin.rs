use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use pkgar_core::{Entry, Header, Mode, dryoc::classic::crypto_sign::crypto_sign_detached};
use pkgar_core::{HeaderFlags, PackageSrc, PublicKey, SecretKey};
#[cfg(not(feature = "repo"))]
use pkgar_keys::PublicKeyFile;

use crate::ext::{DataWriter, EntryExt, PackageSrcExt, copy_and_hash};
#[cfg(not(feature = "repo"))]
use crate::package::PackageFile;
#[cfg(feature = "repo")]
use crate::repo::{GenericPackage, open_or_download_pubkey};
use crate::transaction::Transaction;
use crate::{Error, READ_WRITE_HASH_BUF_SIZE, wrap_io_err};

/// Iterate a directory and return its entries
pub fn folder_entries<P>(base: P) -> Result<Vec<Entry>, Error>
where
    P: AsRef<Path>,
{
    let mut entries = Vec::new();
    folder_entries_inner(&base, &base, &mut entries)?;
    Ok(entries)
}

fn folder_entries_inner<P, Q>(base: P, path: Q, entries: &mut Vec<Entry>) -> Result<(), Error>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let base = base.as_ref();
    let path = path.as_ref();

    // Sort each folder's entries by the file name
    let mut read_dir = Vec::new();
    for entry_res in fs::read_dir(path).map_err(wrap_io_err!(path, "Reading entries"))? {
        let Ok(entry) = entry_res else {
            continue;
        };
        read_dir.push(entry);
    }

    read_dir.sort_by_key(|path| path.file_name());

    for entry in read_dir {
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(wrap_io_err!(entry_path, "Getting entry metadata"))?;
        let entry_type = metadata.file_type();
        if entry_type.is_dir() {
            folder_entries_inner(base, entry_path, entries)?;
        } else {
            let Ok(relative) = entry_path.strip_prefix(base) else {
                continue;
            };
            let mode = Mode::new_file(
                metadata.permissions().mode(),
                entry_type.is_file(),
                entry_type.is_symlink(),
            )?;
            entries.push(Entry::new_uninit(relative.as_os_str().as_bytes(), mode)?);
        }
    }

    Ok(())
}

/// Create a new pkgar file with given secret and source path
pub fn create(
    secret_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    source_path: impl AsRef<Path>,
) -> Result<(), Error> {
    create_with_flags(
        secret_path,
        archive_path,
        source_path,
        HeaderFlags::latest(
            pkgar_core::Architecture::Independent,
            pkgar_core::Packaging::Uncompressed,
        ),
    )
}

/// Create a new pkgar file with given secret and source path and header flags
pub fn create_with_flags(
    secret_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    source_path: impl AsRef<Path>,
    flags: HeaderFlags,
) -> Result<(), Error> {
    let keyfile = pkgar_keys::get_skey(secret_path.as_ref())?;
    let Some(secret_key) = keyfile.secret_key() else {
        return Err(Error::Keys(pkgar_keys::Error::PassphraseRequired));
    };
    let Some(public_key) = keyfile.public_key() else {
        return Err(Error::Keys(pkgar_keys::Error::PassphraseRequired));
    };

    let entries = folder_entries(&source_path)?;

    create_with_entries(
        secret_key,
        public_key,
        archive_path,
        flags,
        entries,
        source_path,
    )
}

/// Create a new pkgar file with extracted secret keys from `pkgar_keys::get_skey` and entries from `folder_entries`
fn create_with_entries(
    secret_key: SecretKey,
    public_key: PublicKey,
    archive_path: impl AsRef<Path>,
    flags: HeaderFlags,
    mut entries: Vec<Entry>,
    source_path: impl AsRef<Path>,
) -> Result<(), Error> {
    let archive_path = archive_path.as_ref();
    let source_path = source_path.as_ref();
    let mut archive_file =
        fs::File::create(archive_path).map_err(wrap_io_err!(archive_path, "Opening source"))?;

    // Create initial header
    let mut header = Header {
        signature: [0; 64],
        public_key,
        blake3: [0; 32],
        count: entries.len() as u32,
        flags,
    };

    let data_offset = header.total_size()?;
    archive_file
        .seek(SeekFrom::Start(data_offset as u64))
        .map_err(wrap_io_err!(archive_path, "Seeking archive file"))?;

    //TODO: fallocate data_offset + data_size

    // Stream each file, writing data and calculating b3sums
    let mut header_hasher = blake3::Hasher::new();
    let mut buf = vec![0; 4 * 1024 * 1024];
    let mut data_offset: u64 = 0;
    for entry in &mut entries {
        let relative = entry.check_path()?;
        let path = source_path.join(relative);

        let mode = entry.mode().map_err(Error::from)?;

        // uncompressed size, compressed size, real size
        let (ulen, clen, rlen, hash) = match mode.kind() {
            Mode::FILE => {
                let mut entry_file = fs::OpenOptions::new()
                    .read(true)
                    .open(&path)
                    .map_err(wrap_io_err!(path, "Opening entry data"))?;
                let entry_meta = entry_file
                    .metadata()
                    .map_err(wrap_io_err!(path, "Checking entry data size"))?;
                let start_pos = archive_file
                    .stream_position()
                    .map_err(wrap_io_err!(path, "Getting file position"))?;
                let rlen = entry_meta.len();
                let mut writer = DataWriter::new(header.flags.packaging(), archive_file, rlen)
                    .map_err(wrap_io_err!(path, "Writing entry data size"))?;
                let (ulen, hash) = copy_and_hash(&mut entry_file, &mut writer, &mut buf)
                    .map_err(wrap_io_err!(path, "Writing data to archive"))?;
                archive_file = writer
                    .finish()
                    .map_err(wrap_io_err!(path, "Finalize archive"))?;
                let end_pos = archive_file
                    .stream_position()
                    .map_err(wrap_io_err!(path, "Getting file position"))?;
                (ulen, end_pos - start_pos, rlen, hash)
            }
            Mode::SYMLINK => {
                let destination =
                    fs::read_link(&path).map_err(wrap_io_err!(path, "Reading entry symlink"))?;
                let start_pos = archive_file
                    .stream_position()
                    .map_err(wrap_io_err!(path, "Getting file position"))?;
                let mut data = destination.as_os_str().as_bytes();
                let rlen = data.len() as u64;
                let mut writer = DataWriter::new(header.flags.packaging(), archive_file, rlen)
                    .map_err(wrap_io_err!(path, "Writing entry data size"))?;
                let (ulen, hash) = copy_and_hash(&mut data, &mut writer, &mut buf)
                    .map_err(wrap_io_err!(path, "Writing data to archive"))?;
                archive_file = writer
                    .finish()
                    .map_err(wrap_io_err!(path, "Finalize archive"))?;
                let end_pos = archive_file
                    .stream_position()
                    .map_err(wrap_io_err!(path, "Getting file position"))?;
                (ulen, end_pos - start_pos, rlen, hash)
            }
            _ => {
                return Err(Error::from(pkgar_core::Error::InvalidMode(mode.bits())));
            }
        };
        if ulen != rlen {
            return Err(Error::LengthMismatch {
                actual: ulen,
                expected: rlen,
            });
        }

        entry.init(hash.into(), data_offset, clen);
        data_offset = data_offset
            .checked_add(clen)
            .ok_or(pkgar_core::Error::Overflow)
            .map_err(Error::from)?;

        header_hasher.update_rayon(bytemuck::bytes_of(entry));
    }
    header
        .blake3
        .copy_from_slice(header_hasher.finalize().as_bytes());

    //TODO: ensure file size matches

    let mut signature = [0; 64];
    crypto_sign_detached(
        &mut signature,
        &bytemuck::bytes_of(&header)[64..],
        &secret_key,
    )
    .map_err(pkgar_core::Error::Dryoc)?;
    header.signature.copy_from_slice(&signature);

    // Write archive header
    archive_file.seek(SeekFrom::Start(0)).map_err(wrap_io_err!(
        archive_path.to_path_buf(),
        "Seeking archive_file back to 0"
    ))?;

    archive_file
        .write_all(bytemuck::bytes_of(&header))
        .map_err(wrap_io_err!(archive_path.to_path_buf(), "Writing header"))?;

    // Write each entry header
    for entry in &entries {
        let _ = entry.check_path()?;
        archive_file
            .write_all(bytemuck::bytes_of(entry))
            .map_err(wrap_io_err!(archive_path.to_path_buf(), "Writing entry"))?;
    }

    archive_file
        .flush()
        .map_err(wrap_io_err!("Flushing archive"))?;

    Ok(())
}

macro_rules! init_package {
    ($pkey_path:expr, $archive_path:expr) => {{
        #[cfg(feature = "repo")]
        let pkey = open_or_download_pubkey($pkey_path)?.pkey;
        #[cfg(not(feature = "repo"))]
        let pkey = PublicKeyFile::open($pkey_path)?.pkey;
        #[cfg(feature = "repo")]
        let package = GenericPackage::open_or_download($archive_path, &pkey)?;
        #[cfg(not(feature = "repo"))]
        let package = PackageFile::new($archive_path, &pkey)?;

        (pkey, package)
    }};
}

/// Extract a pkgar file to a directory
pub fn extract(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);

    Transaction::install(&mut package, base_dir)?.commit()?;

    Ok(())
}

/// Update a directory from a pkgar file
pub fn replace(
    old_pkey_path: impl AsRef<Path>,
    pkey_path: impl AsRef<Path>,
    old_head_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
) -> Result<(), Error> {
    let (_, mut old_package) = init_package!(&old_pkey_path, &old_head_path);
    let (_, mut new_package) = init_package!(&pkey_path, &archive_path);

    Transaction::replace(&mut old_package, &mut new_package, base_dir)?.commit()?;

    Ok(())
}

/// Remove directory entries from a pkgar file
pub fn remove(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);

    Transaction::remove(&mut package, base_dir)?.commit()?;

    Ok(())
}

/// Extract a portion of pkgar file to a directory
pub fn extract_with_subpath(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
    subpath: impl AsRef<Path>,
    strip: bool,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);
    let entries = filter_entries(subpath, strip, &mut package)?;

    let mut transaction = Transaction::new();
    transaction.install_with_entries(&mut package, &entries, base_dir, true)?;
    transaction.commit()?;

    Ok(())
}

/// Update a directory from a pkgar file
pub fn replace_with_subpath(
    old_pkey_path: impl AsRef<Path>,
    pkey_path: impl AsRef<Path>,
    old_head_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
    subpath: impl AsRef<Path>,
    strip: bool,
) -> Result<(), Error> {
    let (_, mut old_package) = init_package!(&old_pkey_path, &old_head_path);
    let (_, mut new_package) = init_package!(&pkey_path, &archive_path);
    let old_entries = filter_entries(subpath.as_ref(), strip, &mut old_package)?;
    let new_entries = filter_entries(subpath.as_ref(), strip, &mut new_package)?;

    let mut transaction = Transaction::new();
    transaction.replace_with_entries(
        &old_entries,
        &new_entries,
        Some(&old_package),
        &mut new_package,
        base_dir,
        false,
    )?;
    transaction.commit()?;
    Ok(())
}

/// Remove a portion of directory entries from a pkgar file
pub fn remove_with_subpath(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
    subpath: impl AsRef<Path>,
    strip: bool,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);
    let entries = filter_entries(subpath, strip, &mut package)?;

    let mut transaction = Transaction::new();
    transaction.remove_with_entries(Some(&mut package), &entries, base_dir, true)?;
    transaction.commit()?;

    Ok(())
}
pub fn list(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    subpath: Option<impl AsRef<Path>>,
    format: Option<&str>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);

    let tabs: Vec<_> = format.unwrap_or("path").split(',').collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut max_widths = vec![4; tabs.len()];

    for entry in package.read_entries()? {
        if let Some(subpath) = subpath.as_ref()
            && entry.sub_path(subpath.as_ref()).is_none()
        {
            continue;
        }
        let relative = entry.check_path()?;
        let mut row = Vec::with_capacity(tabs.len());

        for (i, tab) in tabs.iter().enumerate() {
            let cell = match *tab {
                "path" => relative.display().to_string(),
                "name" => {
                    let name = relative.file_name().unwrap_or_default();
                    Path::new(name).display().to_string()
                }
                "size" => {
                    let size = entry.size;
                    size.to_string()
                }
                "offset" => {
                    let offset = entry.offset;
                    offset.to_string()
                }
                "mode" => {
                    let mode = entry.mode;
                    format!("{:o}", mode)
                }
                _ => String::new(),
            };

            max_widths[i] = max_widths[i].max(cell.len());
            row.push(cell);
        }
        rows.push(row);
    }

    // TODO: option for no header
    {
        for (i, cell) in tabs.iter().enumerate() {
            if i == tabs.len() - 1 {
                print!("{}", cell);
            } else {
                print!("{:<width$}  ", cell, width = max_widths[i]);
            }
        }
        println!();
    }

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i == tabs.len() - 1 {
                print!("{}", cell);
            } else {
                print!("{:<width$}  ", cell, width = max_widths[i]);
            }
        }
        println!();
    }

    Ok(())
}

/// Print a pkgar file content
pub fn cat(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    path: impl AsRef<Path>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);
    let path = path.as_ref();

    let entries = package.read_entries()?;
    let Some(entry) = entries
        .into_iter()
        .find(|s| s.check_path().is_ok_and(|s| s == path))
    else {
        return Err(Error::Io {
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
            path: Some(path.to_path_buf()),
            context: "Entry not found",
        });
    };

    let mut reader = package.data_reader(&entry)?;
    copy_and_hash(
        &mut reader,
        &mut std::io::stdout(),
        &mut vec![0; READ_WRITE_HASH_BUF_SIZE],
    )
    .map_err(wrap_io_err!(archive_path.as_ref(), "Writing to stdout"))?;
    reader.finish(&mut package)?;

    Ok(())
}

/// Split a package file into head and optional data
pub fn split(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    head_path: impl AsRef<Path>,
    data_path_opt: Option<impl AsRef<Path>>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);

    package.split(head_path, data_path_opt)
}

/// Split a package file into head and optional data
pub fn verify(
    pkey_path: impl AsRef<Path>,
    archive_path: impl AsRef<Path>,
    base_dir: impl AsRef<Path>,
) -> Result<(), Error> {
    let (_, mut package) = init_package!(&pkey_path, &archive_path);

    package.verify(base_dir)
}

fn filter_entries(
    subpath: impl AsRef<Path>,
    strip: bool,
    package: &mut impl PackageSrc<Err = Error>,
) -> Result<Vec<Entry>, Error> {
    let entries = package.read_entries()?;
    let subpath = subpath.as_ref();
    let entries: Vec<_> = if strip {
        entries
            .into_iter()
            .filter_map(|s| s.sub_path(subpath))
            .collect()
    } else {
        entries
            .into_iter()
            .filter(|s| s.sub_path(subpath).is_some())
            .collect()
    };
    Ok(entries)
}
