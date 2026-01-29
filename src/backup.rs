use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Archive, Builder};

use crate::app::AppFactory;

const BACKUP_FILES: &[&str] = &["bookmarks.csv", "config.yaml", "workspaces.yaml"];
const BACKUP_DIRS: &[&str] = &["uploads"];

/// Write target for backup: either a file path or stdout (when piped).
enum BackupTarget {
    File(PathBuf),
    Stdout,
}

pub fn create_backup(output_path: Option<PathBuf>) -> Result<()> {
    let paths = AppFactory::get_paths()?;
    let base_path = Path::new(&paths.base_path);

    let target = match output_path {
        Some(p) => BackupTarget::File(p),
        None if !io::stdout().is_terminal() => BackupTarget::Stdout,
        None => {
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
            BackupTarget::File(PathBuf::from(format!("bb-backup-{timestamp}.tar.gz")))
        }
    };

    // Use stderr for progress when writing to stdout
    let piped = matches!(target, BackupTarget::Stdout);

    let writer: Box<dyn Write> = match &target {
        BackupTarget::File(path) => {
            let file = File::create(path)
                .with_context(|| format!("Failed to create archive at {}", path.display()))?;
            Box::new(file)
        }
        BackupTarget::Stdout => Box::new(io::stdout().lock()),
    };

    let encoder = GzEncoder::new(writer, Compression::default());
    let mut archive = Builder::new(encoder);

    let mut included_count = 0;

    for filename in BACKUP_FILES {
        let file_path = base_path.join(filename);
        if file_path.exists() {
            archive
                .append_path_with_name(&file_path, filename)
                .with_context(|| format!("Failed to add {filename} to archive"))?;
            log_progress(piped, &format!("  + {filename}"));
            included_count += 1;
        }
    }

    for dirname in BACKUP_DIRS {
        let dir_path = base_path.join(dirname);
        if dir_path.exists() && dir_path.is_dir() {
            append_dir_recursive(&mut archive, &dir_path, Path::new(dirname), piped)?;
            included_count += 1;
        }
    }

    if included_count == 0 {
        anyhow::bail!("No files found to backup in {}", base_path.display());
    }

    let encoder = archive
        .into_inner()
        .context("Failed to finalize tar archive")?;
    encoder.finish().context("Failed to finalize gzip stream")?;

    if let BackupTarget::File(path) = &target {
        let metadata = std::fs::metadata(path)?;
        let size_kb = metadata.len() / 1024;
        log_progress(piped, &format!("\nBackup created: {} ({} KB)", path.display(), size_kb));
    }

    Ok(())
}

/// Print progress to stdout normally, or stderr when piped.
fn log_progress(piped: bool, msg: &str) {
    if piped {
        eprintln!("{msg}");
    } else {
        println!("{msg}");
    }
}

fn append_dir_recursive<W: Write>(
    archive: &mut Builder<W>,
    source_dir: &Path,
    archive_prefix: &Path,
    piped: bool,
) -> Result<()> {
    for entry in std::fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read directory {}", source_dir.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let archive_path = archive_prefix.join(&file_name);

        if entry_path.is_dir() {
            append_dir_recursive(archive, &entry_path, &archive_path, piped)?;
        } else {
            archive
                .append_path_with_name(&entry_path, &archive_path)
                .with_context(|| format!("Failed to add {} to archive", entry_path.display()))?;
            log_progress(piped, &format!("  + {}", archive_path.display()));
        }
    }
    Ok(())
}

pub fn import_backup(archive_path: &Path, skip_confirm: bool) -> Result<()> {
    let paths = AppFactory::get_paths()?;
    let base_path = Path::new(&paths.base_path);

    // Open and validate archive
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive at {}", archive_path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // First pass: validate archive contains expected files
    let entries = archive
        .entries()
        .context("Failed to read archive entries")?;

    let mut valid_entries: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.context("Failed to read archive entry")?;
        let entry_path = entry.path().context("Failed to get entry path")?;
        let entry_str = entry_path.to_string_lossy().to_string();

        if is_whitelisted(&entry_str) {
            valid_entries.push(entry_str);
        }
    }

    if valid_entries.is_empty() {
        anyhow::bail!(
            "Archive does not contain any recognized backup files.\n\
             Expected: {:?} or files under {:?}",
            BACKUP_FILES,
            BACKUP_DIRS
        );
    }

    println!("Found {} files to import:", valid_entries.len());
    for entry in &valid_entries {
        println!("  {entry}");
    }
    println!("\nDestination: {}", base_path.display());

    // Confirm unless --yes
    if !skip_confirm {
        println!("\nThis will overwrite existing files. Continue? [y/N] ");
        let stdin = std::io::stdin();
        let mut line = String::new();
        BufReader::new(stdin.lock())
            .read_line(&mut line)
            .context("Failed to read user input")?;

        let response = line.trim().to_lowercase();
        if response != "y" && response != "yes" {
            println!("Import cancelled.");
            return Ok(());
        }
    }

    // Second pass: extract whitelisted entries
    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut imported_count = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        if !is_whitelisted(&entry_path) {
            continue;
        }

        let dest_path = base_path.join(&entry_path);

        // Ensure parent directory exists
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        entry
            .unpack(&dest_path)
            .with_context(|| format!("Failed to extract {entry_path}"))?;

        println!("  + {entry_path}");
        imported_count += 1;
    }

    println!("\nImported {imported_count} files to {}", base_path.display());

    Ok(())
}

fn is_whitelisted(path: &str) -> bool {
    // Check if it's a known file
    if BACKUP_FILES.contains(&path) {
        return true;
    }

    // Check if it's under a known directory
    for dir in BACKUP_DIRS {
        if path.starts_with(&format!("{dir}/")) {
            return true;
        }
    }

    false
}
