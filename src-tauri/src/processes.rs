//! Managed child processes, stdin protocol, monitoring and update-install exclusion.

use crate::dummy::resolve_dummy_target;
use crate::library::load_library;
use crate::models::{ExitedPayload, LaunchRequest, LaunchResult, QuestTick};
use crate::settings::is_spanish;
use crate::state::{AppState, DATA_OPERATIONS, UPDATING};
use crate::timers::{QuestClock, QuestTimerConfig, start_quest_timer};
use crate::tray::update_tray_menu;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

static NEXT_TIMER_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct RunningDummy {
    pub(crate) child: Child,
    pub(crate) stdin: ChildStdin,
    pub(crate) name: String,
    pub(crate) timer_stop: Option<Arc<AtomicBool>>,
    pub(crate) quest_clock: Option<Arc<Mutex<QuestClock>>>,
    pub(crate) last_tick: Option<Arc<Mutex<Option<QuestTick>>>>,
}

impl Drop for RunningDummy {
    fn drop(&mut self) {
        if let Some(stop) = &self.timer_stop {
            stop.store(true, Ordering::SeqCst);
        }
    }
}

pub(crate) fn running_keys(state: &AppState) -> Result<Vec<String>, String> {
    let mut running = state.running.lock().map_err(|error| error.to_string())?;
    let keys: Vec<String> = running.keys().cloned().collect();
    let mut active = Vec::new();

    for key in keys {
        let keep = match running.get_mut(&key) {
            Some(dummy) => matches!(dummy.child.try_wait(), Ok(None)),
            None => false,
        };
        if keep {
            active.push(key);
        } else {
            running.remove(&key);
        }
    }

    Ok(active)
}

pub(crate) fn stop_running_dummy(mut dummy: RunningDummy) {
    if let Some(timer_stop) = dummy.timer_stop.take() {
        timer_stop.store(true, Ordering::SeqCst);
    }
    let _ = dummy.stdin.write_all(b"QUIT\n");
    let _ = dummy.stdin.flush();
    thread::sleep(Duration::from_millis(80));

    if matches!(dummy.child.try_wait(), Ok(None)) {
        let _ = dummy.child.kill();
    }
    let _ = dummy.child.wait();
}

pub(crate) fn stop_all_internal(state: &AppState) {
    if let Ok(mut running) = state.running.lock() {
        for (_, dummy) in running.drain() {
            stop_running_dummy(dummy);
        }
    }
}

pub(crate) fn send_dummy_command(state: &AppState, key: &str, command: &str) {
    if let Ok(mut running) = state.running.lock()
        && let Some(dummy) = running.get_mut(key)
    {
        let _ = dummy.stdin.write_all(command.as_bytes());
        let _ = dummy.stdin.flush();
    }
}

pub(crate) fn spawn_monitor(app: AppHandle, running: Arc<Mutex<HashMap<String, RunningDummy>>>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(500));

            let mut exited = Vec::new();
            {
                let Ok(mut running) = running.lock() else {
                    break;
                };
                let keys: Vec<String> = running.keys().cloned().collect();

                for key in keys {
                    let Some(dummy) = running.get_mut(&key) else {
                        continue;
                    };

                    match dummy.child.try_wait() {
                        Ok(Some(status)) => {
                            exited.push(ExitedPayload {
                                key: key.clone(),
                                name: dummy.name.clone(),
                                code: status.code(),
                            });
                            running.remove(&key);
                        }
                        Ok(None) => {}
                        Err(_) => {
                            exited.push(ExitedPayload {
                                key: key.clone(),
                                name: dummy.name.clone(),
                                code: None,
                            });
                            running.remove(&key);
                        }
                    }
                }
            }

            let had_exits = !exited.is_empty();
            for payload in exited {
                let _ = app.emit("dummy://exited", payload);
            }
            // Replacing the native menu while it is open makes Windows dismiss it instantly,
            // so only rebuild it when a dummy actually exits.
            if had_exits {
                let state = app.state::<AppState>();
                update_tray_menu(&app, state.inner());
            }
        }
    });
}

pub(crate) fn launch_dummy(
    app: AppHandle,
    state: &AppState,
    request: LaunchRequest,
) -> Result<LaunchResult, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    if UPDATING.load(Ordering::SeqCst) {
        return Err("An update is being installed".into());
    }
    let app_id = request.app_id.trim().to_string();
    let exe = request.exe.trim().to_string();

    if app_id.is_empty() || exe.is_empty() {
        return Err("App id and executable are required".to_string());
    }

    let library = load_library(&state.paths.my_games_path)?;
    let game = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .cloned()
        .ok_or_else(|| "Game not found in library".to_string())?;
    let name = game.name.clone();
    let game_key = format!("{}::{}", game.app_id, game.exe);

    {
        let mut running = state.running.lock().map_err(|error| error.to_string())?;
        if let Some(existing) = running.get_mut(&game_key) {
            match existing.child.try_wait() {
                Ok(None) => {
                    return Ok(LaunchResult {
                        game_key,
                        exe_path: String::new(),
                        already_running: true,
                    });
                }
                _ => {
                    running.remove(&game_key);
                }
            }
        }
    }

    let (destination, working_dir) = resolve_dummy_target(&app, &state.paths, &game)?;
    let settings = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    let mut command = Command::new(&destination);
    command
        .arg(&name)
        .arg("--managed")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(&working_dir);

    if settings.show_dummy_window {
        command.arg("--window");
    } else {
        command.arg("--hidden");
    }
    if !settings.minimize_to_tray {
        command.arg("--no-tray");
    }

    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Could not open dummy stdin".to_string())?;

    let (timer_stop, quest_clock, last_tick) = if settings.quest_timer_enabled {
        let timer_id = NEXT_TIMER_ID.fetch_add(1, Ordering::Relaxed);
        let handles = start_quest_timer(
            app.clone(),
            game_key.clone(),
            name.clone(),
            timer_id,
            QuestTimerConfig {
                target_minutes: settings.quest_duration_minutes,
                buffer_seconds: settings.quest_sync_buffer_seconds,
                auto_stop: settings.auto_stop_on_complete,
                notify: settings.notify_on_complete,
                spanish: is_spanish(&settings),
            },
        );
        (
            Some(handles.stop),
            Some(handles.clock),
            Some(handles.last_tick),
        )
    } else {
        (None, None, None)
    };

    let dummy = RunningDummy {
        child,
        stdin,
        name,
        timer_stop,
        quest_clock,
        last_tick,
    };

    state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .insert(game_key.clone(), dummy);

    update_tray_menu(&app, state);

    let _ = app.emit(
        "dummy://started",
        ExitedPayload {
            key: game_key.clone(),
            name: game.name,
            code: None,
        },
    );

    Ok(LaunchResult {
        game_key,
        exe_path: destination.to_string_lossy().to_string(),
        already_running: false,
    })
}

pub(crate) fn stop_dummy(
    app: AppHandle,
    state: &AppState,
    game_key: String,
) -> Result<bool, String> {
    let dummy = state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&game_key);

    if let Some(dummy) = dummy {
        let name = dummy.name.clone();
        stop_running_dummy(dummy);
        update_tray_menu(&app, state);
        let _ = app.emit(
            "dummy://exited",
            ExitedPayload {
                key: game_key,
                name,
                code: None,
            },
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(crate) fn stop_all_dummies(app: AppHandle, state: &AppState) -> Result<usize, String> {
    let mut running = state.running.lock().map_err(|error| error.to_string())?;
    let count = running.len();
    let entries: Vec<_> = running.drain().collect();
    drop(running);
    for (key, dummy) in entries {
        let name = dummy.name.clone();
        stop_running_dummy(dummy);
        let _ = app.emit(
            "dummy://exited",
            ExitedPayload {
                key,
                name,
                code: None,
            },
        );
    }
    update_tray_menu(&app, state);
    Ok(count)
}

pub(crate) fn list_running_dummies(state: &AppState) -> Result<Vec<String>, String> {
    running_keys(state)
}

pub(crate) fn set_update_installing(state: &AppState, installing: bool) -> Result<(), String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    if installing && !running_keys(state)?.is_empty() {
        return Err("Stop all active dummies before installing an update".into());
    }
    UPDATING.store(installing, Ordering::SeqCst);
    Ok(())
}
