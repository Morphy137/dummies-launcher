//! Native tray menus and window restoration for launcher-managed dummies.

use crate::models::ExitedPayload;
use crate::processes::{send_dummy_command, stop_all_internal, stop_running_dummy};
use crate::settings::is_spanish;
use crate::state::AppState;
use std::thread;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub(crate) fn update_tray_menu(app: &AppHandle, state: &AppState) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    let spanish = state
        .settings
        .lock()
        .map(|settings| is_spanish(&settings))
        .unwrap_or(false);
    let text = |en, es| if spanish { es } else { en };
    let mut entries: Vec<(String, String)> = {
        let Ok(running) = state.running.lock() else {
            return;
        };
        running
            .iter()
            .map(|(key, dummy)| (key.clone(), dummy.name.clone()))
            .collect()
    };
    entries.sort_by_key(|(_, name)| name.to_lowercase());

    let build = || -> tauri::Result<()> {
        let menu = Menu::new(app)?;
        let open = MenuItem::with_id(
            app,
            "open",
            text("Open Dummies", "Abrir Dummies"),
            true,
            None::<&str>,
        )?;
        menu.append(&open)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;

        if entries.is_empty() {
            let empty = MenuItem::with_id(
                app,
                "empty",
                text("No games running", "No hay dummies activos"),
                false,
                None::<&str>,
            )?;
            menu.append(&empty)?;
        } else {
            for (key, name) in &entries {
                let submenu = Submenu::with_id(app, format!("game:{key}"), name, true)?;
                let show = MenuItem::with_id(
                    app,
                    format!("show:{key}"),
                    text("Show / restore window", "Mostrar / restaurar ventana"),
                    true,
                    None::<&str>,
                )?;
                let hide = MenuItem::with_id(
                    app,
                    format!("hide:{key}"),
                    text("Hide window", "Ocultar ventana"),
                    true,
                    None::<&str>,
                )?;
                let stop = MenuItem::with_id(
                    app,
                    format!("stop:{key}"),
                    text("Stop game", "Detener dummy"),
                    true,
                    None::<&str>,
                )?;
                submenu.append(&show)?;
                submenu.append(&hide)?;
                submenu.append(&PredefinedMenuItem::separator(app)?)?;
                submenu.append(&stop)?;
                menu.append(&submenu)?;
            }
        }

        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let stop_all = MenuItem::with_id(
            app,
            "stop_all",
            text("Stop all games", "Detener todos"),
            !entries.is_empty(),
            None::<&str>,
        )?;
        menu.append(&stop_all)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let quit = MenuItem::with_id(app, "quit", text("Quit", "Salir"), true, None::<&str>)?;
        menu.append(&quit)?;

        tray.set_menu(Some(menu))?;
        let tooltip = if entries.is_empty() {
            "Dummies".to_string()
        } else {
            format!("Dummies — {} {}", entries.len(), text("running", "activos"))
        };
        tray.set_tooltip(Some(tooltip))?;
        Ok(())
    };

    if let Err(error) = build() {
        log::error!("Tray menu: {error}");
    }
}

fn handle_tray_menu(app: &AppHandle, id: &str) {
    if id == "open" {
        show_main_window(app);
        return;
    }

    if id == "quit" {
        let state = app.state::<AppState>();
        stop_all_internal(state.inner());
        app.exit(0);
        return;
    }

    if id == "stop_all" {
        let state = app.state::<AppState>();
        let keys: Vec<String> = {
            let Ok(mut running) = state.running.lock() else {
                return;
            };
            let keys = running.keys().cloned().collect();
            for (_, dummy) in running.drain() {
                stop_running_dummy(dummy);
            }
            keys
        };
        for key in keys {
            let _ = app.emit(
                "dummy://exited",
                ExitedPayload {
                    key,
                    name: String::new(),
                    code: None,
                },
            );
        }
        update_tray_menu(app, state.inner());
        return;
    }

    if let Some(key) = id.strip_prefix("show:") {
        let state = app.state::<AppState>();
        send_dummy_command(state.inner(), key, "SHOW\n");
        return;
    }

    if let Some(key) = id.strip_prefix("hide:") {
        let state = app.state::<AppState>();
        send_dummy_command(state.inner(), key, "HIDE\n");
        return;
    }

    if let Some(key) = id.strip_prefix("stop:") {
        let state = app.state::<AppState>();
        let dummy = state
            .running
            .lock()
            .ok()
            .and_then(|mut running| running.remove(key));
        if let Some(dummy) = dummy {
            stop_running_dummy(dummy);
            let _ = app.emit(
                "dummy://exited",
                ExitedPayload {
                    key: key.to_string(),
                    name: String::new(),
                    code: None,
                },
            );
            update_tray_menu(app, state.inner());
        }
    }
}

pub(crate) fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id("main")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app = app.clone();
            let id = event.id().as_ref().to_string();
            thread::spawn(move || handle_tray_menu(&app, &id));
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    let state = app.state::<AppState>();
    update_tray_menu(app, state.inner());
    Ok(())
}
