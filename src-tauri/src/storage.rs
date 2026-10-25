use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_WRITE: AtomicU64 = AtomicU64::new(1);

pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "Cannot read {}: {error}. Restore its .bak file.",
                path.display()
            )
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(format!("Cannot read {}: {error}", path.display())),
    }
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    // Never replace corrupt data (or its last good backup) with defaults.
    if path.exists() {
        let _: serde_json::Value = read_json(path)?;
    }
    write_atomic(path, &bytes, true)
}

pub fn write_atomic(path: &Path, bytes: &[u8], backup: bool) -> Result<(), String> {
    let temporary = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if backup && path.exists() {
            let previous = fs::read(path)?;
            write_atomic(&path.with_extension("json.bak"), &previous, false)
                .map_err(std::io::Error::other)?;
        }
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result.map_err(|error| format!("Cannot save {}: {error}", path.display()))
}
