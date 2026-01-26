//! Binary storage for vector embeddings.
//!
//! File format: vectors.bin
//!
//! Header (47 bytes):
//! - version: u8 (1)
//! - model_id: [u8; 32] (SHA256 hash of model name)
//! - dimensions: u16 (little-endian)
//! - entry_count: u64 (little-endian)
//! - checksum: u32 (CRC32 of header fields before checksum)
//!
//! Entries (repeated):
//! - bookmark_id: u64 (little-endian)
//! - content_hash: u64 (little-endian)
//! - embedding: [f32; dimensions] (little-endian)

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::semantic::index::{VectorEntry, VectorIndex};

/// Current file format version
const FORMAT_VERSION: u8 = 1;

/// Header size in bytes: version(1) + model_id(32) + dimensions(2) + entry_count(8) + checksum(4)
const HEADER_SIZE: usize = 47;

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum VectorStorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("Version mismatch: file version {0}, supported version {1}")]
    VersionMismatch(u8, u8),

    #[error("Model mismatch: file uses different model")]
    ModelMismatch,

    #[error("Checksum mismatch: file may be corrupted")]
    ChecksumMismatch,

    #[error("Dimension mismatch: expected {expected}, file has {got}")]
    DimensionMismatch { expected: usize, got: usize },
}

/// Storage manager for vector embeddings.
pub struct VectorStorage {
    path: PathBuf,
}

impl VectorStorage {
    /// Create a new storage manager for the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Get the storage file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if the storage file exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Load the vector index from storage.
    ///
    /// # Arguments
    /// * `expected_model_id` - SHA256 hash of the expected model name
    /// * `expected_dimensions` - Expected embedding dimensions
    ///
    /// # Returns
    /// A populated VectorIndex, or an error if the file is invalid/incompatible.
    pub fn load(
        &self,
        expected_model_id: &[u8; 32],
        expected_dimensions: usize,
    ) -> Result<VectorIndex, VectorStorageError> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        // Read and validate header
        let header = self.read_header(&mut reader)?;
        self.validate_header(&header, expected_model_id, expected_dimensions)?;

        // Create index and load entries
        let mut index = VectorIndex::with_capacity(header.dimensions as usize, header.entry_count as usize);

        for _ in 0..header.entry_count {
            let (id, content_hash, embedding) = self.read_entry(&mut reader, header.dimensions as usize)?;
            // Skip entries that fail to insert (e.g., zero norm)
            let _ = index.insert(id, content_hash, embedding);
        }

        Ok(index)
    }

    /// Save the vector index to storage.
    ///
    /// Uses atomic write: temp file -> fsync -> rename
    pub fn save(
        &self,
        index: &VectorIndex,
        model_id: &[u8; 32],
    ) -> Result<(), VectorStorageError> {
        let temp_path = self.path.with_extension("tmp");

        // Write to temp file
        let result = self.write_to_file(&temp_path, index, model_id);

        if result.is_err() {
            // Clean up temp file on error
            let _ = std::fs::remove_file(&temp_path);
            return result;
        }

        // Atomic rename
        std::fs::rename(&temp_path, &self.path)?;

        Ok(())
    }

    /// Delete the storage file if it exists.
    pub fn delete(&self) -> Result<(), VectorStorageError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    /// Write index to a file.
    fn write_to_file(
        &self,
        path: &Path,
        index: &VectorIndex,
        model_id: &[u8; 32],
    ) -> Result<(), VectorStorageError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header
        let header = Header {
            version: FORMAT_VERSION,
            model_id: *model_id,
            dimensions: index.dimensions() as u16,
            entry_count: index.len() as u64,
            checksum: 0, // Will be computed
        };
        self.write_header(&mut writer, &header)?;

        // Write entries
        for (id, entry) in index.iter() {
            self.write_entry(&mut writer, id, entry)?;
        }

        // Flush and sync
        writer.flush()?;
        let file = writer.into_inner().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        file.sync_all()?;

        Ok(())
    }

    /// Read header from file.
    fn read_header(&self, reader: &mut BufReader<File>) -> Result<Header, VectorStorageError> {
        let mut header_bytes = [0u8; HEADER_SIZE];
        reader.read_exact(&mut header_bytes)?;

        let version = header_bytes[0];

        // Version check first
        if version > FORMAT_VERSION {
            return Err(VectorStorageError::VersionMismatch(version, FORMAT_VERSION));
        }

        let mut model_id = [0u8; 32];
        model_id.copy_from_slice(&header_bytes[1..33]);

        let dimensions = u16::from_le_bytes([header_bytes[33], header_bytes[34]]);
        let entry_count = u64::from_le_bytes([
            header_bytes[35],
            header_bytes[36],
            header_bytes[37],
            header_bytes[38],
            header_bytes[39],
            header_bytes[40],
            header_bytes[41],
            header_bytes[42],
        ]);
        let stored_checksum = u32::from_le_bytes([
            header_bytes[43],
            header_bytes[44],
            header_bytes[45],
            header_bytes[46],
        ]);

        // Verify checksum (computed over header without checksum field)
        let computed_checksum = Self::compute_checksum(&header_bytes[0..43]);
        if stored_checksum != computed_checksum {
            return Err(VectorStorageError::ChecksumMismatch);
        }

        Ok(Header {
            version,
            model_id,
            dimensions,
            entry_count,
            checksum: stored_checksum,
        })
    }

    /// Validate header against expected values.
    fn validate_header(
        &self,
        header: &Header,
        expected_model_id: &[u8; 32],
        expected_dimensions: usize,
    ) -> Result<(), VectorStorageError> {
        if header.model_id != *expected_model_id {
            return Err(VectorStorageError::ModelMismatch);
        }

        if header.dimensions as usize != expected_dimensions {
            return Err(VectorStorageError::DimensionMismatch {
                expected: expected_dimensions,
                got: header.dimensions as usize,
            });
        }

        Ok(())
    }

    /// Write header to file.
    fn write_header(&self, writer: &mut BufWriter<File>, header: &Header) -> Result<(), VectorStorageError> {
        let mut header_bytes = [0u8; HEADER_SIZE];

        header_bytes[0] = header.version;
        header_bytes[1..33].copy_from_slice(&header.model_id);
        header_bytes[33..35].copy_from_slice(&header.dimensions.to_le_bytes());
        header_bytes[35..43].copy_from_slice(&header.entry_count.to_le_bytes());

        // Compute and store checksum
        let checksum = Self::compute_checksum(&header_bytes[0..43]);
        header_bytes[43..47].copy_from_slice(&checksum.to_le_bytes());

        writer.write_all(&header_bytes)?;
        Ok(())
    }

    /// Read a single entry from file.
    fn read_entry(
        &self,
        reader: &mut BufReader<File>,
        dimensions: usize,
    ) -> Result<(u64, u64, Vec<f32>), VectorStorageError> {
        // Read bookmark_id
        let mut id_bytes = [0u8; 8];
        reader.read_exact(&mut id_bytes)?;
        let id = u64::from_le_bytes(id_bytes);

        // Read content_hash
        let mut hash_bytes = [0u8; 8];
        reader.read_exact(&mut hash_bytes)?;
        let content_hash = u64::from_le_bytes(hash_bytes);

        // Read embedding
        let mut embedding = Vec::with_capacity(dimensions);
        for _ in 0..dimensions {
            let mut float_bytes = [0u8; 4];
            reader.read_exact(&mut float_bytes)?;
            embedding.push(f32::from_le_bytes(float_bytes));
        }

        Ok((id, content_hash, embedding))
    }

    /// Write a single entry to file.
    fn write_entry(
        &self,
        writer: &mut BufWriter<File>,
        id: u64,
        entry: &VectorEntry,
    ) -> Result<(), VectorStorageError> {
        writer.write_all(&id.to_le_bytes())?;
        writer.write_all(&entry.content_hash.to_le_bytes())?;

        for &value in &entry.embedding {
            writer.write_all(&value.to_le_bytes())?;
        }

        Ok(())
    }

    /// Compute CRC32 checksum of data.
    fn compute_checksum(data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }
}

/// File header structure.
#[derive(Debug)]
struct Header {
    version: u8,
    model_id: [u8; 32],
    dimensions: u16,
    entry_count: u64,
    checksum: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "bb-vectors-test-{}-{}.bin",
            std::process::id(),
            counter
        ))
    }

    fn test_model_id() -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0] = 0xAB;
        id[31] = 0xCD;
        id
    }

    #[test]
    fn test_save_and_load_empty() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let index = VectorIndex::new(384);
        storage.save(&index, &model_id).unwrap();

        assert!(storage.exists());

        let loaded = storage.load(&model_id, 384).unwrap();
        assert_eq!(loaded.len(), 0);
        assert_eq!(loaded.dimensions(), 384);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_and_load_with_entries() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let mut index = VectorIndex::new(3);
        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        index.insert(2, 200, vec![0.0, 1.0, 0.0]).unwrap();
        index.insert(3, 300, vec![0.0, 0.0, 1.0]).unwrap();

        storage.save(&index, &model_id).unwrap();

        let loaded = storage.load(&model_id, 3).unwrap();
        assert_eq!(loaded.len(), 3);

        let entry1 = loaded.get(1).unwrap();
        assert_eq!(entry1.content_hash, 100);
        assert_eq!(entry1.embedding, vec![1.0, 0.0, 0.0]);

        let entry2 = loaded.get(2).unwrap();
        assert_eq!(entry2.content_hash, 200);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_model_mismatch() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let index = VectorIndex::new(3);
        storage.save(&index, &model_id).unwrap();

        // Try to load with different model ID
        let mut wrong_model_id = [0u8; 32];
        wrong_model_id[0] = 0xFF;

        let result = storage.load(&wrong_model_id, 3);
        assert!(matches!(result, Err(VectorStorageError::ModelMismatch)));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_dimension_mismatch() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let index = VectorIndex::new(3);
        storage.save(&index, &model_id).unwrap();

        // Try to load with different dimensions
        let result = storage.load(&model_id, 384);
        assert!(matches!(result, Err(VectorStorageError::DimensionMismatch { .. })));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_atomic_write_cleans_up_on_error() {
        let path = PathBuf::from("/nonexistent/directory/vectors.bin");
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let index = VectorIndex::new(3);
        let result = storage.save(&index, &model_id);

        assert!(result.is_err());
        // Temp file should be cleaned up
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn test_delete() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let index = VectorIndex::new(3);
        storage.save(&index, &model_id).unwrap();
        assert!(storage.exists());

        storage.delete().unwrap();
        assert!(!storage.exists());
    }

    #[test]
    fn test_checksum_detects_corruption() {
        let path = temp_path();
        let storage = VectorStorage::new(path.clone());
        let model_id = test_model_id();

        let mut index = VectorIndex::new(3);
        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        storage.save(&index, &model_id).unwrap();

        // Corrupt the file
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap();
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(10)).unwrap();
        file.write_all(&[0xFF]).unwrap();

        // Load should fail with checksum error
        let result = storage.load(&model_id, 3);
        assert!(matches!(result, Err(VectorStorageError::ChecksumMismatch)));

        let _ = std::fs::remove_file(&path);
    }
}
