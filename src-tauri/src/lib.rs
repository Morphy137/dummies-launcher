//! Application composition: plugin setup, managed state, IPC registration and shutdown.

use crate::catalog::load_database;
use crate::migration::{migrate_dummy_window_visibility, migrate_legacy_data};
use crate::processes::{spawn_monitor, stop_all_internal};
use crate::settings::load_settings;
use crate::shortcuts::dispatch_shortcut;
use crate::state::{AppPaths, AppState};
use crate::tray::{create_tray, show_main_window};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{Manager, RunEvent};

mod bootstrap;
mod catalog;
mod commands;
mod dummy;
#[cfg(test)]
mod ipc_contract_tests;
mod library;
mod migration;
mod models;
mod paths;
mod processes;
#[cfg(test)]
mod regression_tests;
mod resources;
mod settings;
mod shortcuts;
mod state;
mod steam;
mod storage;
#[cfg(test)]
mod tests;
mod timers;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            show_main_window(app);
            dispatch_shortcut(app.clone(), args);
        }))
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Resized(_))
                && window.is_minimized().unwrap_or(false)
            {
                let _ = window.hide();
            }
        })
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;
            app.handle().plugin(tauri_plugin_notification::init())?;
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.handle().plugin(tauri_plugin_process::init())?;

            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let games_dir = data_dir.join("games");
            fs::create_dir_all(&games_dir)?;
            let paths = AppPaths {
                games_dir,
                settings_path: data_dir.join("settings.json"),
                my_games_path: data_dir.join("myGames.json"),
                gamelist_path: data_dir.join("gamelist.json"),
            };
            migrate_legacy_data(&paths);
            migrate_dummy_window_visibility(&paths);
            let settings = load_settings(&paths.settings_path);
            let database = load_database(&paths.gamelist_path);

            let state = AppState {
                paths,
                settings: Mutex::new(settings),
                database: RwLock::new(database),
                running: Arc::new(Mutex::new(HashMap::new())),
            };
            let running = state.running.clone();
            app.manage(state);
            create_tray(app.handle())?;
            spawn_monitor(app.handle().clone(), running);
            dispatch_shortcut(app.handle().clone(), std::env::args().collect());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_system_locale,
            commands::get_bootstrap,
            commands::sync_gamelist,
            commands::get_database_page,
            commands::get_library,
            commands::add_game,
            commands::remove_game,
            commands::toggle_favorite,
            commands::update_game_executable,
            commands::create_shortcut,
            commands::get_steam_path,
            commands::get_steam_libraries,
            commands::setup_steam_integration,
            commands::remove_steam_integration,
            commands::launch_dummy,
            commands::stop_dummy,
            commands::pause_dummy,
            commands::resume_dummy,
            commands::stop_all_dummies,
            commands::list_running_dummies,
            commands::set_update_installing,
            commands::report_frontend_error
        ])
        .build(tauri::generate_context!())
        .expect("error while building Dummies")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. }) {
                let state = app_handle.state::<AppState>();
                stop_all_internal(state.inner());
            }
        });
}
