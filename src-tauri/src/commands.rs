//! Stable IPC adapters. Validation, locking and side effects belong to the services.

use crate::models::{
    AddGameRequest, Bootstrap, DatabasePage, LaunchRequest, LaunchResult, LibraryGame, Settings,
    SteamRemoveResult, SteamSetupRequest, SteamSetupResult, SyncResult,
};
use crate::state::AppState;
use crate::{bootstrap, catalog, library, processes, settings, shortcuts, steam, timers};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    settings::get_settings(state.inner())
}

#[tauri::command(async)]
pub(crate) fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    settings::save_settings(app, state.inner(), settings)
}

#[tauri::command]
pub(crate) fn get_system_locale() -> String {
    settings::get_system_locale()
}

#[tauri::command(async)]
pub(crate) fn get_bootstrap(state: State<AppState>) -> Result<Bootstrap, String> {
    bootstrap::get_bootstrap(state.inner())
}

#[tauri::command(async)]
pub(crate) fn sync_gamelist(state: State<'_, AppState>) -> Result<SyncResult, String> {
    catalog::sync_gamelist(state.inner())
}

#[tauri::command(async)]
pub(crate) fn get_database_page(
    state: State<AppState>,
    filter: String,
    offset: usize,
    limit: usize,
) -> Result<DatabasePage, String> {
    catalog::get_database_page(state.inner(), filter, offset, limit)
}

#[tauri::command(async)]
pub(crate) fn get_library(state: State<AppState>) -> Result<Vec<LibraryGame>, String> {
    library::get_library(state.inner())
}

#[tauri::command(async)]
pub(crate) fn add_game(
    state: State<AppState>,
    request: AddGameRequest,
) -> Result<LibraryGame, String> {
    library::add_game(state.inner(), request)
}

#[tauri::command(async)]
pub(crate) fn remove_game(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<bool, String> {
    library::remove_game(state.inner(), app_id, exe)
}

#[tauri::command(async)]
pub(crate) fn toggle_favorite(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<LibraryGame, String> {
    library::toggle_favorite(state.inner(), app_id, exe)
}

#[tauri::command(async)]
pub(crate) fn update_game_executable(
    state: State<AppState>,
    app_id: String,
    old_exe: String,
    new_exe: String,
) -> Result<LibraryGame, String> {
    library::update_game_executable(state.inner(), app_id, old_exe, new_exe)
}

#[tauri::command]
pub(crate) fn get_steam_path() -> Option<String> {
    steam::get_steam_path()
}

#[tauri::command(async)]
pub(crate) fn get_steam_libraries() -> Vec<String> {
    steam::get_steam_libraries()
}

#[tauri::command(async)]
pub(crate) fn setup_steam_integration(
    app: AppHandle,
    state: State<AppState>,
    request: SteamSetupRequest,
) -> Result<SteamSetupResult, String> {
    steam::setup_steam_integration(app, state.inner(), request)
}

#[tauri::command(async)]
pub(crate) fn remove_steam_integration(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<SteamRemoveResult, String> {
    steam::remove_steam_integration(state.inner(), app_id, exe)
}

#[tauri::command(async)]
pub(crate) fn create_shortcut(
    app: AppHandle,
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<String, String> {
    shortcuts::create_shortcut(app, state.inner(), app_id, exe)
}

#[tauri::command(async)]
pub(crate) fn launch_dummy(
    app: AppHandle,
    state: State<AppState>,
    request: LaunchRequest,
) -> Result<LaunchResult, String> {
    processes::launch_dummy(app, state.inner(), request)
}

#[tauri::command(async)]
pub(crate) fn stop_dummy(
    app: AppHandle,
    state: State<AppState>,
    game_key: String,
) -> Result<bool, String> {
    processes::stop_dummy(app, state.inner(), game_key)
}

#[tauri::command]
pub(crate) fn pause_dummy(state: State<AppState>, game_key: String) -> Result<bool, String> {
    timers::pause_dummy(state.inner(), game_key)
}

#[tauri::command]
pub(crate) fn resume_dummy(state: State<AppState>, game_key: String) -> Result<bool, String> {
    timers::resume_dummy(state.inner(), game_key)
}

#[tauri::command(async)]
pub(crate) fn stop_all_dummies(app: AppHandle, state: State<AppState>) -> Result<usize, String> {
    processes::stop_all_dummies(app, state.inner())
}

#[tauri::command]
pub(crate) fn list_running_dummies(state: State<AppState>) -> Result<Vec<String>, String> {
    processes::list_running_dummies(state.inner())
}

#[tauri::command(async)]
pub(crate) fn set_update_installing(
    state: State<AppState>,
    installing: bool,
) -> Result<(), String> {
    processes::set_update_installing(state.inner(), installing)
}

#[tauri::command]
pub(crate) fn report_frontend_error(message: String) {
    log::error!(
        "UI operation: {}",
        message.chars().take(2000).collect::<String>()
    );
}
