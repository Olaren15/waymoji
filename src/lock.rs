use std::fs::{File, OpenOptions, TryLockError};
use std::path::PathBuf;

pub fn acquire_lock() -> std::io::Result<Option<File>> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = runtime_dir.join("waymoji.lock");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;

    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => {
            if std::env::var_os("WAYMOJI_DEBUG").is_some() {
                eprintln!("[waymoji] another instance is already running");
            }
            Ok(None)
        }
        Err(TryLockError::Error(err)) => Err(err),
    }
}
