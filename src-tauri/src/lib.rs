use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    theme: String,
    language: String,
    show_dummy_window: bool,
    minimize_to_tray: bool,
    cards_per_row: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "system".to_string(),
            show_dummy_window: false,
            minimize_to_tray: true,
            cards_per_row: 6,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    settings: Settings,
    system_locale: String,
    running_keys: Vec<String>,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchRequest {
    app_id: String,
    name: String,
    exe: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchResult {
    game_key: String,
    exe_path: String,
    already_running: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitedPayload {
    key: String,
    name: String,
    code: Option<i32>,
}

struct AppPaths {
    games_dir: PathBuf,
    settings_path: PathBuf,
}

struct RunningDummy {
    child: Child,
    stdin: ChildStdin,
    name: String,
}

struct AppState {
    paths: AppPaths,
    settings: Mutex<Settings>,
    running: Arc<Mutex<HashMap<String, RunningDummy>>>,
}

fn load_settings(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Settings>(&content).ok())
        .map(normalize_settings)
        .unwrap_or_default()
}

fn normalize_settings(mut settings: Settings) -> Settings {
    if !matches!(settings.theme.as_str(), "dark" | "light") {
        settings.theme = "system".to_string();
    }
    if !matches!(settings.language.as_str(), "en" | "es") {
        settings.language = "system".to_string();
    }
    if !(4..=8).contains(&settings.cards_per_row) {
        settings.cards_per_row = 6;
    }
    settings
}

fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn sanitize_component(value: &str) -> Option<String> {
    let mut cleaned = String::new();

    for character in value.chars() {
        if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        {
            cleaned.push('_');
        } else {
            cleaned.push(character);
        }
    }

    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_relative_exe(value: &str) -> Option<PathBuf> {
    let normalized = value.replace('\\', "/");
    let mut path = PathBuf::new();

    for component in normalized.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            return None;
        }
        path.push(sanitize_component(component)?);
    }

    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn find_dummy_source(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("DUMMY_GAME_EXE") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        for candidate in [
            resource_dir.join("dummy-game.exe"),
            resource_dir.join("resources").join("dummy-game.exe"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let candidate = manifest_dir
            .join("..")
            .join("crates")
            .join("dummy-game")
            .join("target")
            .join(profile)
            .join("dummy-game.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn prepare_dummy(
    source: &Path,
    paths: &AppPaths,
    app_id: &str,
    exe: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let app_folder = sanitize_component(app_id).ok_or_else(|| "Invalid app id".to_string())?;
    let exe_relative =
        sanitize_relative_exe(exe).ok_or_else(|| "Invalid executable path".to_string())?;
    let exe_name = exe_relative
        .file_name()
        .ok_or_else(|| "Invalid executable name".to_string())?;
    let exe_parent = exe_relative.parent().unwrap_or_else(|| Path::new(""));
    let working_dir = paths.games_dir.join(app_folder).join(exe_parent);
    let destination = working_dir.join(exe_name);

    fs::create_dir_all(&working_dir).map_err(|error| error.to_string())?;

    let should_copy = match (fs::metadata(source), fs::metadata(&destination)) {
        (Ok(source_meta), Ok(destination_meta)) => source_meta.len() != destination_meta.len(),
        _ => true,
    };

    if should_copy {
        fs::copy(source, &destination).map_err(|error| error.to_string())?;
    }

    Ok((destination, working_dir))
}

fn running_keys(state: &AppState) -> Result<Vec<String>, String> {
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

fn stop_running_dummy(mut dummy: RunningDummy) {
    let _ = dummy.stdin.write_all(b"QUIT\n");
    let _ = dummy.stdin.flush();
    thread::sleep(Duration::from_millis(80));

    if matches!(dummy.child.try_wait(), Ok(None)) {
        let _ = dummy.child.kill();
    }
    let _ = dummy.child.wait();
}

fn stop_all_internal(state: &AppState) {
    if let Ok(mut running) = state.running.lock() {
        for (_, dummy) in running.drain() {
            stop_running_dummy(dummy);
        }
    }
}

fn spawn_monitor(app: AppHandle, running: Arc<Mutex<HashMap<String, RunningDummy>>>) {
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

            for payload in exited {
                let _ = app.emit("dummy://exited", payload);
            }
        }
    });
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<Settings, String> {
    let normalized = normalize_settings(settings);
    write_settings(&state.paths.settings_path, &normalized)?;
    *state.settings.lock().map_err(|error| error.to_string())? = normalized.clone();
    Ok(normalized)
}

#[tauri::command]
fn get_system_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_string())
}

#[tauri::command]
fn get_bootstrap(state: State<AppState>) -> Result<Bootstrap, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    Ok(Bootstrap {
        settings,
        system_locale: get_system_locale(),
        running_keys: running_keys(&state)?,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tauri::command]
fn launch_dummy(
    app: AppHandle,
    state: State<AppState>,
    request: LaunchRequest,
) -> Result<LaunchResult, String> {
    let app_id = request.app_id.trim().to_string();
    let name = request.name.trim().to_string();
    let exe = request.exe.trim().to_string();

    if app_id.is_empty() || name.is_empty() || exe.is_empty() {
        return Err("App id, name and executable are required".to_string());
    }

    let game_key = format!("{app_id}::{exe}");

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

    let source = find_dummy_source(&app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    let (destination, working_dir) = prepare_dummy(&source, &state.paths, &app_id, &exe)?;
    let settings = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    let mut command = Command::new(&destination);
    command
        .arg(&name)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(&working_dir);

    if settings.show_dummy_window {
        command.arg("--window");
    }
    if !settings.minimize_to_tray {
        command.arg("--no-tray");
    }

    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Could not open dummy stdin".to_string())?;

    let dummy = RunningDummy { child, stdin, name };

    state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .insert(game_key.clone(), dummy);

    Ok(LaunchResult {
        game_key,
        exe_path: destination.to_string_lossy().to_string(),
        already_running: false,
    })
}

#[tauri::command]
fn stop_dummy(state: State<AppState>, game_key: String) -> Result<bool, String> {
    let dummy = state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&game_key);

    if let Some(dummy) = dummy {
        stop_running_dummy(dummy);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn stop_all_dummies(state: State<AppState>) -> Result<usize, String> {
    let mut running = state.running.lock().map_err(|error| error.to_string())?;
    let count = running.len();
    for (_, dummy) in running.drain() {
        stop_running_dummy(dummy);
    }
    Ok(count)
}

#[tauri::command]
fn list_running_dummies(state: State<AppState>) -> Result<Vec<String>, String> {
    running_keys(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let games_dir = data_dir.join("games");
            fs::create_dir_all(&games_dir)?;
            let settings_path = data_dir.join("settings.json");
            let settings = load_settings(&settings_path);

            let state = AppState {
                paths: AppPaths {
                    games_dir,
                    settings_path,
                },
                settings: Mutex::new(settings),
                running: Arc::new(Mutex::new(HashMap::new())),
            };
            let running = state.running.clone();
            app.manage(state);
            spawn_monitor(app.handle().clone(), running);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_system_locale,
            get_bootstrap,
            launch_dummy,
            stop_dummy,
            stop_all_dummies,
            list_running_dummies
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("dummies-{label}-{unique}"))
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_relative_exe("../escape.exe").is_none());
        assert!(sanitize_relative_exe("Binaries/../../escape.exe").is_none());
        assert!(sanitize_component("..").is_none());
        assert!(sanitize_component("").is_none());
    }

    #[test]
    fn keeps_valid_relative_executable_paths() {
        let path = sanitize_relative_exe("Binaries/Win64/game.exe").expect("valid path");
        assert_eq!(
            path,
            PathBuf::from("Binaries").join("Win64").join("game.exe")
        );
    }

    #[test]
    fn normalizes_unknown_settings() {
        let settings = normalize_settings(Settings {
            theme: "neon".to_string(),
            language: "fr".to_string(),
            show_dummy_window: true,
            minimize_to_tray: false,
            cards_per_row: 99,
        });

        assert_eq!(settings.theme, "system");
        assert_eq!(settings.language, "system");
        assert_eq!(settings.cards_per_row, 6);
        assert!(settings.show_dummy_window);
        assert!(!settings.minimize_to_tray);
    }

    #[test]
    fn writes_and_reads_settings() {
        let directory = temporary_directory("settings");
        fs::create_dir_all(&directory).expect("create temp dir");
        let path = directory.join("settings.json");
        let settings = Settings {
            theme: "dark".to_string(),
            language: "es".to_string(),
            show_dummy_window: true,
            minimize_to_tray: false,
            cards_per_row: 7,
        };

        write_settings(&path, &settings).expect("write settings");
        let loaded = load_settings(&path);

        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "es");
        assert_eq!(loaded.cards_per_row, 7);
        assert!(loaded.show_dummy_window);
        assert!(!loaded.minimize_to_tray);

        fs::remove_dir_all(directory).expect("cleanup temp dir");
    }

    #[test]
    fn copies_dummy_into_the_game_folder() {
        let directory = temporary_directory("dummy");
        let source = directory.join("source.exe");
        let games = directory.join("games");
        fs::create_dir_all(&games).expect("create games dir");
        fs::write(&source, b"dummy").expect("write source");

        let paths = AppPaths {
            games_dir: games.clone(),
            settings_path: directory.join("settings.json"),
        };
        let (destination, working_dir) =
            prepare_dummy(&source, &paths, "123", "Binaries/Win64/game.exe").expect("prepare");

        assert!(destination.is_file());
        assert_eq!(fs::read(&destination).expect("read destination"), b"dummy");
        assert_eq!(
            working_dir,
            games.join("123").join("Binaries").join("Win64")
        );

        fs::remove_dir_all(directory).expect("cleanup temp dir");
    }
}
