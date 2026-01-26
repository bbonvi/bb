//! File locking for mutual exclusion between daemon and CLI instances.
//!
//! Uses flock() for advisory locking on the database directory.
//! - Daemon: acquires exclusive lock on startup, holds for lifetime
//! - CLI standalone: acquires exclusive lock per-operation
//! - CLI-to-daemon: skips locking (daemon holds the lock)

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// Lock file name placed in the base directory
const LOCK_FILE_NAME: &str = "bb.lock";

/// A held file lock that releases on drop
pub struct FileLock {
    #[allow(dead_code)]
    file: File,
}

impl FileLock {
    /// Attempt to acquire an exclusive lock on the database directory.
    /// Returns `Ok(FileLock)` if acquired, or an error if locked by another process.
    pub fn try_acquire(base_path: &Path) -> io::Result<Self> {
        let lock_path = base_path.join(LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        Self::try_lock_exclusive(&file)?;

        Ok(FileLock { file })
    }

    /// Acquire an exclusive lock, blocking until available.
    pub fn acquire_blocking(base_path: &Path) -> io::Result<Self> {
        let lock_path = base_path.join(LOCK_FILE_NAME);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        Self::lock_exclusive(&file)?;

        Ok(FileLock { file })
    }

    #[cfg(unix)]
    fn try_lock_exclusive(file: &File) -> io::Result<()> {
        let fd = file.as_raw_fd();
        let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock
                || err.raw_os_error() == Some(libc::EWOULDBLOCK)
                || err.raw_os_error() == Some(libc::EAGAIN)
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Database is locked by another process (daemon or CLI)",
                ));
            }
            return Err(err);
        }
        Ok(())
    }

    #[cfg(unix)]
    fn lock_exclusive(file: &File) -> io::Result<()> {
        let fd = file.as_raw_fd();
        let result = unsafe { libc::flock(fd, libc::LOCK_EX) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn try_lock_exclusive(_file: &File) -> io::Result<()> {
        // On non-Unix platforms, we don't implement locking (yet)
        // This allows the code to compile but provides no protection
        Ok(())
    }

    #[cfg(not(unix))]
    fn lock_exclusive(_file: &File) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for FileLock {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        // Release the lock - ignore errors on drop
        unsafe { libc::flock(fd, libc::LOCK_UN) };
    }
}

/// Check if CLI should skip locking (when connecting to daemon via BB_ADDR)
pub fn should_skip_locking() -> bool {
    std::env::var("BB_ADDR").is_ok()
}

/// Guard that optionally holds a lock.
/// Use this for CLI operations that may or may not need locking.
pub enum LockGuard {
    Held(FileLock),
    Skipped,
}

impl LockGuard {
    /// Acquire lock if not using remote backend, skip otherwise.
    pub fn acquire_if_local(base_path: &Path) -> io::Result<Self> {
        if should_skip_locking() {
            Ok(LockGuard::Skipped)
        } else {
            FileLock::try_acquire(base_path).map(LockGuard::Held)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bb-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_acquire_and_release() {
        let dir = temp_dir();

        // First lock should succeed
        let lock1 = FileLock::try_acquire(&dir);
        assert!(lock1.is_ok(), "First lock should succeed");

        // Second lock should fail (non-blocking)
        let lock2 = FileLock::try_acquire(&dir);
        assert!(lock2.is_err(), "Second lock should fail");

        // Drop first lock
        drop(lock1);

        // Now third lock should succeed
        let lock3 = FileLock::try_acquire(&dir);
        assert!(lock3.is_ok(), "Third lock should succeed after release");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_skip_locking_when_remote() {
        // Without BB_ADDR, should not skip
        std::env::remove_var("BB_ADDR");
        assert!(!should_skip_locking());

        // With BB_ADDR, should skip
        std::env::set_var("BB_ADDR", "http://localhost:8080");
        assert!(should_skip_locking());
        std::env::remove_var("BB_ADDR");
    }

    #[test]
    fn test_lock_guard_skips_for_remote() {
        let dir = temp_dir();

        std::env::set_var("BB_ADDR", "http://localhost:8080");
        let guard = LockGuard::acquire_if_local(&dir);
        assert!(matches!(guard, Ok(LockGuard::Skipped)));
        std::env::remove_var("BB_ADDR");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
