//! Windows desktop shortcuts and single-instance launch-argument dispatch.

use crate::dummy::resolve_dummy_target;
use crate::library::load_library;
use crate::models::LaunchRequest;
use crate::paths::sanitize_component;
use crate::processes::launch_dummy;
use crate::state::{AppState, DATA_OPERATIONS};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn create_shortcut(
    app: AppHandle,
    state: &AppState,
    app_id: String,
    exe: String,
) -> Result<String, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let library = load_library(&state.paths.my_games_path)?;
    let game = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    resolve_dummy_target(&app, &state.paths, game)?;
    let destination = std::env::current_exe().map_err(|error| error.to_string())?;
    let desktop = app
        .path()
        .desktop_dir()
        .map_err(|error| error.to_string())?;
    let base_name = sanitize_component(&game.name).unwrap_or_else(|| "Game".to_string());
    let mut shortcut_path = desktop.join(format!("{base_name}.lnk"));

    if shortcut_path.exists() {
        for index in 2..1000 {
            let candidate = desktop.join(format!("{base_name} ({index}).lnk"));
            if !candidate.exists() {
                shortcut_path = candidate;
                break;
            }
        }
    }

    let mut link = mslnk::ShellLink::new(&destination).map_err(|error| error.to_string())?;
    link.set_name(Some(game.name.clone()));
    link.set_arguments(Some(format!(
        "--launch \"{}\" \"{}\"",
        game.app_id, game.exe
    )));
    link.set_working_dir(
        destination
            .parent()
            .map(|dir| dir.to_string_lossy().to_string()),
    );
    link.create_lnk(&shortcut_path)
        .map_err(|error| error.to_string())?;

    Ok(shortcut_path.to_string_lossy().to_string())
}

pub(crate) fn dispatch_shortcut(app: AppHandle, args: Vec<String>) {
    if args.get(1).map(String::as_str) != Some("--launch") {
        return;
    }
    let (Some(app_id), Some(exe)) = (args.get(2), args.get(3)) else {
        return;
    };
    let request = LaunchRequest {
        app_id: app_id.clone(),
        exe: exe.clone(),
    };
    thread::spawn(move || {
        if let Err(error) = launch_dummy(app.clone(), app.state::<AppState>().inner(), request) {
            log::error!("Shortcut launch failed: {error}");
            let _ = app.emit("dummy://error", error);
        }
    });
}
