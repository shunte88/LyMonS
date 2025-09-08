
use log::{debug, error};
use std::fs::{File, OpenOptions};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct SingleInstance {
    file: Option<File>,
    locked: Arc<AtomicBool>,
}

#[derive(Debug, thiserror::Error)]
pub enum SingleInstanceError {
    #[error("Another instance is already running")]
    AlreadyRunning,
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to create lock directory")]
    LockDirCreation,
}

impl SingleInstance {
    pub fn new(client_name: &str) -> Result<SingleInstance, SingleInstanceError> {
        let cache_dir = cache_dir().ok_or(SingleInstanceError::LockDirCreation)?;
        let lock_dir = cache_dir.join("lymons").join("_locks");
        std::fs::create_dir_all(&lock_dir).map_err(|_| SingleInstanceError::LockDirCreation)?;

        let lockfile = lock_dir.join(client_name);
        // use simple flock
        match OpenOptions::new().write(true).create(true).open(&lockfile) {
            Ok(file) => match file.try_lock_exclusive() {
                Ok(true) => Ok(SingleInstance {
                    file: Some(file),
                    locked: Arc::new(AtomicBool::new(true)),
                }),
                Ok(false) => Err(SingleInstanceError::AlreadyRunning),
                Err(e) => Err(SingleInstanceError::Io(e)),
            },
            Err(e) => Err(SingleInstanceError::Io(e)),
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if self.locked.load(Ordering::SeqCst) {
            //drop the file handle and lock on Unix and Windows
            self.file.take();
            self.locked.store(false, Ordering::SeqCst);
        }
    }
}