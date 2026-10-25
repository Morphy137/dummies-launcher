//! Local template preparation and verified Steam/local launch-target resolution.

use crate::models::LibraryGame;
use crate::paths::{
    ensure_existing_within, reject_redirects, safe_path_component, sanitize_relative_exe,
};
use crate::resources::find_dummy_source;
use crate::state::AppPaths;
use crate::steam::{checked_steam_library, read_owned_artifact, steam_layout};
use crate::storage;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub(crate) fn prepare_dummy(
    source: &Path,
    paths: &AppPaths,
    app_id: &str,
    exe: &str,
) -> Result<(PathBuf, PathBuf), String> {
    if !safe_path_component(app_id) {
        return Err("Invalid app id".into());
    }
    let app_folder = app_id;
    let exe_relative =
        sanitize_relative_exe(exe).ok_or_else(|| "Invalid executable path".to_string())?;
    let exe_name = exe_relative
        .file_name()
        .ok_or_else(|| "Invalid executable name".to_string())?;
    let exe_parent = exe_relative.parent().unwrap_or_else(|| Path::new(""));
    let working_dir = paths.games_dir.join(app_folder).join(exe_parent);
    let destination = working_dir.join(exe_name);

    reject_redirects(&working_dir)?;
    fs::create_dir_all(&working_dir).map_err(|error| error.to_string())?;

    ensure_existing_within(&paths.games_dir, &working_dir)?;
    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    if destination.exists() {
        ensure_existing_within(&working_dir, &destination)?;
    }
    if fs::read(&destination).ok().as_deref() != Some(bytes.as_slice()) {
        storage::write_atomic(&destination, &bytes, false)?;
    }

    Ok((destination, working_dir))
}

pub(crate) fn resolve_dummy_target(
    app: &AppHandle,
    paths: &AppPaths,
    game: &LibraryGame,
) -> Result<(PathBuf, PathBuf), String> {
    if game.use_steam_path {
        let steam_path = checked_steam_library(game.steam_library_path.as_deref())?;
        let layout = steam_layout(
            &steam_path,
            game.steam_app_id.as_deref().unwrap_or_default(),
            game.install_dir.as_deref().unwrap_or_default(),
            &game.exe,
        )?;
        ensure_existing_within(&layout.common, &layout.install_root)?;
        read_owned_artifact(
            &layout.steam_apps,
            &layout.manifest_path,
            game.steam_manifest_sha256.as_deref(),
        )?
        .ok_or_else(|| "The configured Steam manifest no longer exists".to_string())?;
        read_owned_artifact(
            &layout.install_root,
            &layout.destination,
            game.steam_dummy_sha256.as_deref(),
        )?
        .ok_or_else(|| "The configured Steam dummy no longer exists".to_string())?;
        let working_dir = layout
            .destination
            .parent()
            .ok_or_else(|| "Invalid executable destination".to_string())?
            .to_path_buf();
        return Ok((layout.destination, working_dir));
    }

    let source = find_dummy_source(app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    prepare_dummy(&source, paths, &game.app_id, &game.exe)
}
