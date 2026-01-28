use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder;

use crate::app::AppFactory;

const BACKUP_FILES: &[&str] = &["bookmarks.csv", "config.yaml", "workspaces.yaml"];
const BACKUP_DIRS: &[&str] = &["uploads"];

pub fn create_backup(output_path: Option<PathBuf>) -> Result<()> {
    let paths = AppFactory::get_paths()?;
    let base_path = Path::new(&paths.base_path);

    let output = output_path.unwrap_or_else(|| {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        PathBuf::from(format!("bb-backup-{timestamp}.tar.gz"))
    });

    let file = File::create(&output)
        .with_context(|| format!("Failed to create archive at {}", output.display()))?;

    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);

    let mut included_count = 0;

    // Add individual files
    for filename in BACKUP_FILES {
        let file_path = base_path.join(filename);
        if file_path.exists() {
            archive
                .append_path_with_name(&file_path, filename)
                .with_context(|| format!("Failed to add {filename} to archive"))?;
            println!("  + {filename}");
            included_count += 1;
        }
    }

    // Add directories recursively
    for dirname in BACKUP_DIRS {
        let dir_path = base_path.join(dirname);
        if dir_path.exists() && dir_path.is_dir() {
            append_dir_recursive(&mut archive, &dir_path, Path::new(dirname))?;
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

    let metadata = std::fs::metadata(&output)?;
    let size_kb = metadata.len() / 1024;

    println!(
        "\nBackup created: {} ({} KB)",
        output.display(),
        size_kb
    );

    Ok(())
}

fn append_dir_recursive<W: Write>(
    archive: &mut Builder<W>,
    source_dir: &Path,
    archive_prefix: &Path,
) -> Result<()> {
    for entry in std::fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read directory {}", source_dir.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let archive_path = archive_prefix.join(&file_name);

        if entry_path.is_dir() {
            append_dir_recursive(archive, &entry_path, &archive_path)?;
        } else {
            archive
                .append_path_with_name(&entry_path, &archive_path)
                .with_context(|| format!("Failed to add {} to archive", entry_path.display()))?;
            println!("  + {}", archive_path.display());
        }
    }
    Ok(())
}
