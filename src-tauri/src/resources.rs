//! Bundled, configured and development dummy-binary discovery.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) fn find_dummy_source(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("DUMMY_GAME_EXE") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        for candidate in [
            resource_dir.join("dummy-game.exe"),
            resource_dir.join("resources").join("dummy-game.exe"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let candidate = manifest_dir
            .join("..")
            .join("crates")
            .join("dummy-game")
            .join("target")
            .join(profile)
            .join("dummy-game.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}
