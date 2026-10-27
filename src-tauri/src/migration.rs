//! Non-destructive legacy imports and one-time settings migrations.

use crate::settings::{load_settings, write_settings};
use crate::state::AppPaths;
use crate::storage;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn migrate_legacy_data(paths: &AppPaths) {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return;
    };
    let base = PathBuf::from(appdata);
    migrate_legacy_from(&base, paths);
}

pub(crate) fn migrate_legacy_from(base: &Path, paths: &AppPaths) {
    let candidates = [
        "discord-fake-game-launcher-ui",
        "Discord Fake Game Launcher",
        "discord-fake-game-launcher",
        "ds-fake-game-launcher",
    ];

    for candidate in candidates {
        let directory = base.join(candidate);
        if !directory.is_dir() {
            continue;
        }

        let files = [
            ("myGames.json", &paths.my_games_path),
            ("settings.json", &paths.settings_path),
            ("gamelist.json", &paths.gamelist_path),
        ];

        for (name, destination) in files {
            if destination.exists() {
                continue;
            }
            let source = directory.join(name);
            if source.is_file() {
                match fs::read(&source) {
                    Ok(bytes) if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() => {
                        if let Err(error) = storage::write_atomic(destination, &bytes, false)
                            .and_then(|_| {
                                storage::write_atomic(
                                    &destination.with_extension("json.bak"),
                                    &bytes,
                                    false,
                                )
                            })
                        {
                            log::error!("Migration failed: {error}");
                        }
                    }
                    _ => log::warn!("Skipping unreadable legacy file: {}", source.display()),
                }
            }
        }
    }
}

pub(crate) fn migrate_dummy_window_visibility(paths: &AppPaths) {
    let marker = paths
        .settings_path
        .with_file_name(".dummy-window-visible-v1");
    if marker.exists() {
        return;
    }

    if paths.settings_path.is_file() {
        let mut settings = load_settings(&paths.settings_path);
        if !settings.show_dummy_window {
            settings.show_dummy_window = true;
            if write_settings(&paths.settings_path, &settings).is_err() {
                return;
            }
        }
    }

    let _ = fs::write(marker, b"1");
}
