//! Settings normalization/persistence and native UI language selection.

use crate::models::Settings;
use crate::state::{AppState, DATA_OPERATIONS};
use crate::storage;
use crate::tray::update_tray_menu;
use std::path::Path;
use tauri::AppHandle;

pub(crate) fn load_settings(path: &Path) -> Settings {
    match storage::read_json::<Settings>(path) {
        Ok(settings) => normalize_settings(settings),
        Err(error) => {
            log::error!("{error}");
            Settings::default()
        }
    }
}

pub(crate) fn normalize_settings(mut settings: Settings) -> Settings {
    settings.schema_version = 1;
    if !matches!(settings.theme.as_str(), "dark" | "light") {
        settings.theme = "system".to_string();
    }
    if !matches!(settings.language.as_str(), "en" | "es") {
        settings.language = "system".to_string();
    }
    if !(4..=8).contains(&settings.cards_per_row) {
        settings.cards_per_row = 6;
    }
    if !(1..=120).contains(&settings.quest_duration_minutes) {
        settings.quest_duration_minutes = 15;
    }
    if settings.quest_sync_buffer_seconds > 300 {
        settings.quest_sync_buffer_seconds = 60;
    }
    settings
}

pub(crate) fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    storage::write_json(path, settings)
}

pub(crate) fn is_spanish(settings: &Settings) -> bool {
    settings.language == "es"
        || (settings.language == "system" && get_system_locale().to_lowercase().starts_with("es"))
}

pub(crate) fn get_settings(state: &AppState) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|error| error.to_string())
}

pub(crate) fn save_settings(
    app: AppHandle,
    state: &AppState,
    settings: Settings,
) -> Result<Settings, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let normalized = normalize_settings(settings);
    write_settings(&state.paths.settings_path, &normalized)?;
    *state.settings.lock().map_err(|error| error.to_string())? = normalized.clone();
    update_tray_menu(&app, state);
    Ok(normalized)
}

pub(crate) fn get_system_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_string())
}
