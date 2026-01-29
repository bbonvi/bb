use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Archive, Builder};

const BACKUP_FILES: &[&str] = &["bookmarks.csv", "config.yaml", "workspaces.yaml"];
const BACKUP_DIRS: &[&str] = &["uploads"];

/// Write target for backup: either a file path or stdout (when piped).
enum BackupTarget {
    File(PathBuf),
    Stdout,
}

pub fn create_backup(output_path: Option<PathBuf>, base_path: &Path) -> Result<()> {

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

pub fn import_backup(archive_path: Option<&Path>, skip_confirm: bool, base_path: &Path) -> Result<()> {
    let _temp_file: Option<tempfile::NamedTempFile>;
    let archive_path = match archive_path {
        Some(p) => p.to_path_buf(),
        None if !io::stdin().is_terminal() => {
            let mut tmp = tempfile::NamedTempFile::new()
                .context("Failed to create temp file for stdin")?;
            io::copy(&mut io::stdin().lock(), &mut tmp)
                .context("Failed to read archive from stdin")?;
            let path = tmp.path().to_path_buf();
            _temp_file = Some(tmp);
            path
        }
        None => anyhow::bail!("No archive path provided. Pipe an archive to stdin or pass a path."),
    };
    let archive_path = archive_path.as_path();

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    /// Helper: create a populated base dir with sample backup files.
    fn populate_base_dir(dir: &Path) {
        std::fs::write(dir.join("bookmarks.csv"), "url,title\nhttp://a.com,A\n").unwrap();
        std::fs::write(dir.join("config.yaml"), "key: value\n").unwrap();
        std::fs::write(dir.join("workspaces.yaml"), "ws: default\n").unwrap();
        let uploads = dir.join("uploads");
        std::fs::create_dir_all(&uploads).unwrap();
        std::fs::write(uploads.join("file1.png"), b"png-data").unwrap();
        std::fs::write(uploads.join("file2.jpg"), b"jpg-data").unwrap();
    }

    /// Helper: list entry paths in a tar.gz archive.
    fn list_archive_entries(archive_path: &Path) -> Vec<String> {
        let file = File::open(archive_path).unwrap();
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn test_is_whitelisted() {
        assert!(is_whitelisted("bookmarks.csv"));
        assert!(is_whitelisted("config.yaml"));
        assert!(is_whitelisted("workspaces.yaml"));
        assert!(is_whitelisted("uploads/file.png"));
        assert!(is_whitelisted("uploads/sub/deep.jpg"));
        assert!(!is_whitelisted("uploads"));
        assert!(!is_whitelisted("evil.sh"));
        assert!(!is_whitelisted("../etc/passwd"));
        assert!(!is_whitelisted("bookmarks.csv.bak"));
    }

    #[test]
    fn test_backup_creates_archive() {
        let base = TempDir::new().unwrap();
        populate_base_dir(base.path());

        let out_dir = TempDir::new().unwrap();
        let archive_path = out_dir.path().join("test.tar.gz");

        create_backup(Some(archive_path.clone()), base.path()).unwrap();

        assert!(archive_path.exists());
        let entries: HashSet<String> = list_archive_entries(&archive_path).into_iter().collect();
        assert!(entries.contains("bookmarks.csv"));
        assert!(entries.contains("config.yaml"));
        assert!(entries.contains("workspaces.yaml"));
        assert!(entries.contains("uploads/file1.png"));
        assert!(entries.contains("uploads/file2.jpg"));
    }

    #[test]
    fn test_backup_empty_dir_errors() {
        let base = TempDir::new().unwrap();
        let out = TempDir::new().unwrap();
        let archive_path = out.path().join("empty.tar.gz");

        let result = create_backup(Some(archive_path), base.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files found"));
    }

    #[test]
    fn test_import_roundtrip() {
        let base_src = TempDir::new().unwrap();
        populate_base_dir(base_src.path());

        let out = TempDir::new().unwrap();
        let archive_path = out.path().join("roundtrip.tar.gz");
        create_backup(Some(archive_path.clone()), base_src.path()).unwrap();

        // Import into a fresh dir
        let base_dst = TempDir::new().unwrap();
        import_backup(Some(archive_path.as_path()), true, base_dst.path()).unwrap();

        // Verify files match
        assert_eq!(
            std::fs::read_to_string(base_dst.path().join("bookmarks.csv")).unwrap(),
            "url,title\nhttp://a.com,A\n"
        );
        assert_eq!(
            std::fs::read_to_string(base_dst.path().join("config.yaml")).unwrap(),
            "key: value\n"
        );
        assert_eq!(
            std::fs::read_to_string(base_dst.path().join("uploads/file1.png")).unwrap(),
            "png-data"
        );
    }

    #[test]
    fn test_import_rejects_empty_archive() {
        // Build an archive containing only a non-whitelisted file
        let tmp = TempDir::new().unwrap();
        let src = TempDir::new().unwrap();
        std::fs::write(src.path().join("evil.sh"), "#!/bin/bash").unwrap();

        let archive_path = tmp.path().join("bad.tar.gz");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);
        builder
            .append_path_with_name(src.path().join("evil.sh"), "evil.sh")
            .unwrap();
        let enc = builder.into_inner().unwrap();
        enc.finish().unwrap();

        let dest = TempDir::new().unwrap();
        let result = import_backup(Some(archive_path.as_path()), true, dest.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not contain any recognized"));
    }

    #[test]
    fn test_import_skips_non_whitelisted_files() {
        // Build an archive with both whitelisted and non-whitelisted entries
        let tmp = TempDir::new().unwrap();
        let src = TempDir::new().unwrap();
        std::fs::write(src.path().join("bookmarks.csv"), "url\nhttp://b.com\n").unwrap();
        std::fs::write(src.path().join("malware.exe"), "bad").unwrap();

        let archive_path = tmp.path().join("mixed.tar.gz");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);
        builder
            .append_path_with_name(src.path().join("bookmarks.csv"), "bookmarks.csv")
            .unwrap();
        builder
            .append_path_with_name(src.path().join("malware.exe"), "malware.exe")
            .unwrap();
        let enc = builder.into_inner().unwrap();
        enc.finish().unwrap();

        let dest = TempDir::new().unwrap();
        import_backup(Some(archive_path.as_path()), true, dest.path()).unwrap();

        assert!(dest.path().join("bookmarks.csv").exists());
        assert!(!dest.path().join("malware.exe").exists());
    }
}
