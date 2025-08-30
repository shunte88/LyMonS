use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn find_squeezelite_shm_path() -> io::Result<PathBuf> {
    let mut best: Option<(PathBuf, SystemTime)> = None;

    for entry in fs::read_dir("/dev/shm")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("squeezelite-") {
            continue;
        }
        let meta = entry.metadata()?;
        let mtime = meta.modified().unwrap_or(UNIX_EPOCH);

        match &mut best {
            None => best = Some((entry.path(), mtime)),
            Some((_, best_time)) if mtime > *best_time => best = Some((entry.path(), mtime)),
            _ => {}
        }
    }

    best
        .map(|(p, _)| p)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no squeezelite shm found in /dev/shm"))
}
