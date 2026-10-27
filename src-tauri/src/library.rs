//! Validated library persistence and serialized identity-preserving mutations.

use crate::models::{AddGameRequest, DatabaseGame, LibraryGame};
use crate::paths::{safe_path_component, sanitize_component, sanitize_relative_exe};
use crate::state::{AppState, DATA_OPERATIONS};
use crate::storage;
use std::collections::HashMap;
use std::path::Path;

pub(crate) fn load_library(path: &Path) -> Result<Vec<LibraryGame>, String> {
    let library: Vec<LibraryGame> = storage::read_json(path)?;
    validate_library(&library)?;
    Ok(library)
}

fn validate_library(library: &[LibraryGame]) -> Result<(), String> {
    let mut keys = std::collections::HashSet::new();
    for game in library {
        let exe = sanitize_relative_exe(&game.exe)
            .ok_or_else(|| "Invalid executable in library; restore its .bak file".to_string())?;
        if !safe_path_component(&game.app_id)
            || game.name.trim().is_empty()
            || !keys.insert(format!(
                "{}::{}",
                game.app_id.to_lowercase(),
                exe.to_string_lossy().to_lowercase()
            ))
        {
            return Err("Invalid or duplicate library entry; restore its .bak file".into());
        }
    }
    Ok(())
}

pub(crate) fn write_library(path: &Path, library: &[LibraryGame]) -> Result<(), String> {
    validate_library(library)?;
    storage::write_json(path, library)
}

pub(crate) fn enrich_library(library: &mut [LibraryGame], database: &[DatabaseGame]) -> bool {
    let index: HashMap<&str, &DatabaseGame> = database
        .iter()
        .map(|game| (game.id.as_str(), game))
        .collect();
    let mut changed = false;

    for game in library {
        let Some(database_game) = index.get(game.app_id.as_str()) else {
            continue;
        };

        if game.cover_image_hash.is_none() && database_game.cover_image_hash.is_some() {
            game.cover_image_hash = database_game.cover_image_hash.clone();
            changed = true;
        }
        if game.icon_hash.is_none() && database_game.icon_hash.is_some() {
            game.icon_hash = database_game.icon_hash.clone();
            changed = true;
        }
        if game.steam_app_id.is_none() && database_game.steam_app_id.is_some() {
            game.steam_app_id = database_game.steam_app_id.clone();
            changed = true;
        }

        let requires_steam = database_game.uses_new_detection
            && game
                .steam_app_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty());
        if game.requires_steam != requires_steam {
            game.requires_steam = requires_steam;
            changed = true;
        }
    }

    changed
}

pub(crate) fn get_library(state: &AppState) -> Result<Vec<LibraryGame>, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    {
        let database = state.database.read().map_err(|error| error.to_string())?;
        if enrich_library(&mut library, &database) {
            write_library(&state.paths.my_games_path, &library)?;
        }
    }
    Ok(library)
}

pub(crate) fn add_game(state: &AppState, request: AddGameRequest) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("Game name is required".to_string());
    }

    let app_id = request.id.trim().to_string();
    let app_id = if app_id.is_empty() {
        sanitize_component(&name).ok_or_else(|| "Invalid app id".to_string())?
    } else {
        app_id
    };
    let exe_path =
        sanitize_relative_exe(&request.exe).ok_or_else(|| "Invalid executable path".to_string())?;
    let exe = exe_path.to_string_lossy().to_string();
    let steam_app_id = request.steam_app_id.filter(|id| !id.trim().is_empty());
    let requires_steam = request.uses_new_detection && steam_app_id.is_some();

    let mut library = load_library(&state.paths.my_games_path)?;
    if let Some(existing) = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
    {
        return Ok(existing.clone());
    }

    let entry = LibraryGame {
        app_id,
        name,
        exe,
        is_favorite: false,
        steam_app_id,
        icon_hash: request.icon_hash,
        cover_image_hash: request.cover_image_hash,
        requires_steam,
        ..LibraryGame::default()
    };

    library.push(entry.clone());
    write_library(&state.paths.my_games_path, &library)?;
    Ok(entry)
}

pub(crate) fn remove_game(state: &AppState, app_id: String, exe: String) -> Result<bool, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    let candidate = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .cloned();
    if let Some(game) = candidate.as_ref() {
        let running_key = format!("{}::{}", game.app_id, game.exe);
        if state
            .running
            .lock()
            .map_err(|error| error.to_string())?
            .contains_key(&running_key)
        {
            return Err("Stop the game before removing it from the library".to_string());
        }
        if game.use_steam_path {
            return Err(
                "Remove the Steam integration before removing this game from the library"
                    .to_string(),
            );
        }
    }

    let before = library.len();
    library.retain(|game| !(game.app_id == app_id && game.exe == exe));
    let removed = library.len() != before;

    if removed {
        write_library(&state.paths.my_games_path, &library)?;
    }

    Ok(removed)
}

pub(crate) fn toggle_favorite(
    state: &AppState,
    app_id: String,
    exe: String,
) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    let game = library
        .iter_mut()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    game.is_favorite = !game.is_favorite;
    let updated = game.clone();
    write_library(&state.paths.my_games_path, &library)?;
    Ok(updated)
}

pub(crate) fn update_game_executable(
    state: &AppState,
    app_id: String,
    old_exe: String,
    new_exe: String,
) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let running_key = format!("{app_id}::{old_exe}");
    if state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .contains_key(&running_key)
    {
        return Err("Stop the game before changing its executable".to_string());
    }

    let mut path =
        sanitize_relative_exe(&new_exe).ok_or_else(|| "Invalid executable path".to_string())?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
    {
        path.set_extension("exe");
    }
    let new_exe = path.to_string_lossy().to_string();

    let mut library = load_library(&state.paths.my_games_path)?;
    let index = library
        .iter()
        .position(|game| game.app_id == app_id && game.exe == old_exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    if library[index].use_steam_path {
        return Err("Remove the Steam integration before changing the game executable".to_string());
    }
    if new_exe == old_exe {
        return Ok(library[index].clone());
    }
    if library
        .iter()
        .any(|game| game.app_id == app_id && game.exe.eq_ignore_ascii_case(&new_exe))
    {
        return Err("Another library entry already uses that executable".to_string());
    }

    library[index].exe = new_exe;
    let updated = library[index].clone();
    write_library(&state.paths.my_games_path, &library)?;
    Ok(updated)
}
