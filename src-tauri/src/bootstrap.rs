//! Consistent startup/reconciliation snapshots for the webview.

use crate::library::{enrich_library, load_library, write_library};
use crate::models::{Bootstrap, Settings};
use crate::processes::running_keys;
use crate::settings::get_system_locale;
use crate::state::{AppState, DATA_OPERATIONS};
use crate::storage;

pub(crate) fn get_bootstrap(state: &AppState) -> Result<Bootstrap, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let _: Settings = storage::read_json(&state.paths.settings_path)?;
    let settings = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let mut library = load_library(&state.paths.my_games_path)?;
    {
        let database = state.database.read().map_err(|error| error.to_string())?;
        if enrich_library(&mut library, &database) {
            write_library(&state.paths.my_games_path, &library)?;
        }
    }

    Ok(Bootstrap {
        settings,
        system_locale: get_system_locale(),
        running_keys: running_keys(state)?,
        library,
        version: env!("CARGO_PKG_VERSION").to_string(),
        quest_ticks: state
            .running
            .lock()
            .map_err(|error| error.to_string())?
            .values()
            .filter_map(|dummy| dummy.last_tick.as_ref()?.lock().ok()?.clone())
            .collect(),
        portable: std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.parent()
                    .map(|dir| dir.join("portable.marker").is_file())
            })
            .unwrap_or(false),
    })
}
