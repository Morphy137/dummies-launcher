use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_notification::NotificationExt;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const DISCORD_DETECTABLE_URL: &str = "https://discord.com/api/applications/detectable";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    theme: String,
    language: String,
    show_dummy_window: bool,
    minimize_to_tray: bool,
    cards_per_row: u8,
    quest_timer_enabled: bool,
    quest_duration_minutes: u32,
    quest_sync_buffer_seconds: u32,
    auto_stop_on_complete: bool,
    notify_on_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "system".to_string(),
            show_dummy_window: false,
            minimize_to_tray: true,
            cards_per_row: 6,
            quest_timer_enabled: true,
            quest_duration_minutes: 15,
            quest_sync_buffer_seconds: 60,
            auto_stop_on_complete: true,
            notify_on_complete: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    settings: Settings,
    system_locale: String,
    running_keys: Vec<String>,
    library: Vec<LibraryGame>,
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct LibraryGame {
    app_id: String,
    name: String,
    exe: String,
    is_favorite: bool,
    steam_app_id: Option<String>,
    icon_hash: Option<String>,
    cover_image_hash: Option<String>,
    requires_steam: bool,
    use_steam_path: bool,
    install_dir: Option<String>,
    steam_exe_path: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for LibraryGame {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            name: String::new(),
            exe: String::new(),
            is_favorite: false,
            steam_app_id: None,
            icon_hash: None,
            cover_image_hash: None,
            requires_steam: false,
            use_steam_path: false,
            install_dir: None,
            steam_exe_path: None,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseGame {
    id: String,
    name: String,
    exe: String,
    is_launcher: bool,
    uses_new_detection: bool,
    steam_app_id: Option<String>,
    icon_hash: Option<String>,
    cover_image_hash: Option<String>,
    #[serde(skip)]
    name_lower: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabasePage {
    items: Vec<DatabaseGame>,
    offset: usize,
    limit: usize,
    has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResult {
    updated: bool,
    count: usize,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    update_available: bool,
    current_version: String,
    latest_version: String,
    release_url: String,
    release_name: String,
    release_notes: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamSetupResult {
    manifest_path: String,
    dest_exe_path: String,
    steam_path: String,
    updated_game: LibraryGame,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddGameRequest {
    id: String,
    name: String,
    exe: String,
    uses_new_detection: bool,
    steam_app_id: Option<String>,
    icon_hash: Option<String>,
    cover_image_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DetectableApp {
    id: Option<String>,
    name: Option<String>,
    executables: Option<Vec<AppExecutable>>,
    icon_hash: Option<String>,
    cover_image_hash: Option<String>,
    third_party_skus: Option<Vec<ThirdPartySku>>,
}

#[derive(Debug, Deserialize)]
struct AppExecutable {
    name: Option<String>,
    os: Option<String>,
    is_launcher: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ThirdPartySku {
    distributor: Option<String>,
    id: Option<String>,
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
    my_games_path: PathBuf,
    gamelist_path: PathBuf,
}

struct RunningDummy {
    child: Child,
    stdin: ChildStdin,
    name: String,
    timer_stop: Option<Arc<AtomicBool>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestTick {
    key: String,
    name: String,
    remaining_seconds: u64,
    phase: String,
    percent: u8,
}

struct AppState {
    paths: AppPaths,
    settings: Mutex<Settings>,
    database: RwLock<Vec<DatabaseGame>>,
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
    if !(1..=120).contains(&settings.quest_duration_minutes) {
        settings.quest_duration_minutes = 15;
    }
    if settings.quest_sync_buffer_seconds > 300 {
        settings.quest_sync_buffer_seconds = 60;
    }
    settings
}

fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn fallback_exe_name(name: &str) -> String {
    let base = sanitize_component(name).unwrap_or_else(|| "Game".to_string());
    if base.to_ascii_lowercase().ends_with(".exe") {
        base
    } else {
        format!("{base}.exe")
    }
}

fn pick_best_executable(app: &DetectableApp) -> Option<&AppExecutable> {
    let executables = app.executables.as_deref().unwrap_or(&[]);
    executables
        .iter()
        .find(|entry| {
            entry
                .os
                .as_deref()
                .is_some_and(|os| os.eq_ignore_ascii_case("win32"))
                && !entry.is_launcher.unwrap_or(false)
                && entry
                    .name
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty())
        })
        .or_else(|| {
            executables.iter().find(|entry| {
                entry
                    .os
                    .as_deref()
                    .is_some_and(|os| os.eq_ignore_ascii_case("win32"))
                    && entry
                        .name
                        .as_deref()
                        .is_some_and(|name| !name.trim().is_empty())
            })
        })
}

fn to_database_games(apps: Vec<DetectableApp>) -> Vec<DatabaseGame> {
    let mut games = Vec::new();

    for app in apps {
        let Some(name) = app.name.clone().filter(|name| !name.trim().is_empty()) else {
            continue;
        };
        let best = pick_best_executable(&app);
        let exe = best
            .and_then(|entry| entry.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| fallback_exe_name(&name));
        let uses_new_detection = best
            .and_then(|entry| entry.name.as_deref())
            .is_none_or(|name| name.trim().is_empty());
        let is_launcher = best.and_then(|entry| entry.is_launcher).unwrap_or(false);
        let steam_app_id = app.third_party_skus.as_ref().and_then(|skus| {
            skus.iter()
                .find(|sku| {
                    sku.distributor
                        .as_deref()
                        .is_some_and(|distributor| distributor.eq_ignore_ascii_case("steam"))
                })
                .and_then(|sku| sku.id.clone())
        });

        games.push(DatabaseGame {
            id: app.id.unwrap_or_default(),
            name_lower: name.to_lowercase(),
            name,
            exe,
            is_launcher,
            uses_new_detection,
            steam_app_id,
            icon_hash: app.icon_hash,
            cover_image_hash: app.cover_image_hash,
        });
    }

    games.sort_by(|left, right| left.name_lower.cmp(&right.name_lower));
    games
}

fn load_database(path: &Path) -> Vec<DatabaseGame> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<DetectableApp>>(&raw).ok())
        .map(to_database_games)
        .unwrap_or_default()
}

fn fetch_gamelist(path: &Path) -> Result<bool, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Dummies/1.0")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(DISCORD_DETECTABLE_URL)
        .header("Accept", "application/json")
        .send()
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Discord API returned {}", response.status()));
    }

    let text = response.text().map_err(|error| error.to_string())?;
    let remote = text.trim();
    let local = fs::read_to_string(path)
        .ok()
        .map(|content| content.trim().to_string());

    if local.as_deref() == Some(remote) {
        return Ok(false);
    }

    if local.is_some() {
        let _ = fs::copy(path, path.with_extension("json.bak"));
    }

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, format!("{remote}\n")).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    Ok(true)
}

fn load_library(path: &Path) -> Vec<LibraryGame> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<LibraryGame>>(&raw).ok())
        .unwrap_or_default()
}

fn write_library(path: &Path, library: &[LibraryGame]) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(library).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn enrich_library(library: &mut [LibraryGame], database: &[DatabaseGame]) -> bool {
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

fn page_database(
    database: &[DatabaseGame],
    filter: &str,
    offset: usize,
    limit: usize,
) -> DatabasePage {
    let term = filter.trim().to_lowercase();
    let limit = limit.clamp(1, 200);
    let mut items = Vec::new();
    let mut matched = 0;
    let mut has_more = false;

    for game in database {
        if !term.is_empty() && !game.name_lower.contains(&term) {
            continue;
        }

        if matched >= offset && items.len() < limit {
            items.push(game.clone());
        }
        matched += 1;

        if items.len() >= limit && matched > offset + limit {
            has_more = true;
            break;
        }
    }

    DatabasePage {
        items,
        offset,
        limit,
        has_more,
    }
}

fn migrate_legacy_data(paths: &AppPaths) {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return;
    };
    let base = PathBuf::from(appdata);
    let candidates = [
        "discord-fake-game-launcher-ui",
        "Discord Fake Game Launcher",
        "discord-fake-game-launcher",
        "ds-fake-game-launcher",
    ];

    for candidate in candidates {
        let directory = base.join(candidate);
        if !directory.is_dir() {
            continue;
        }

        let files = [
            ("myGames.json", &paths.my_games_path),
            ("settings.json", &paths.settings_path),
            ("gamelist.json", &paths.gamelist_path),
        ];
        let mut copied = false;

        for (name, destination) in files {
            if destination.exists() {
                continue;
            }
            let source = directory.join(name);
            if source.is_file() && fs::copy(&source, destination).is_ok() {
                copied = true;
            }
        }

        if copied {
            break;
        }
    }
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

fn stop_all_internal(state: &AppState) {
    if let Ok(mut running) = state.running.lock() {
        for (_, dummy) in running.drain() {
            stop_running_dummy(dummy);
        }
    }
}

fn start_quest_timer(
    app: AppHandle,
    key: String,
    name: String,
    target_minutes: u32,
    buffer_seconds: u32,
    auto_stop: bool,
    notify: bool,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_signal = stop.clone();

    thread::spawn(move || {
        let start = Instant::now();
        let target = u64::from(target_minutes) * 60;
        let total = target + u64::from(buffer_seconds);

        loop {
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }

            let elapsed = start.elapsed().as_secs();
            let remaining = total.saturating_sub(elapsed);
            let phase = if elapsed >= total {
                "complete"
            } else if elapsed >= target {
                "syncing"
            } else {
                "playing"
            };
            let percent = (elapsed.min(total) * 100).checked_div(total).unwrap_or(100) as u8;

            let _ = app.emit(
                "quest://tick",
                QuestTick {
                    key: key.clone(),
                    name: name.clone(),
                    remaining_seconds: remaining,
                    phase: phase.to_string(),
                    percent,
                },
            );

            if phase == "complete" {
                if notify {
                    let _ = app
                        .notification()
                        .builder()
                        .title("Quest complete")
                        .body(format!("{name} reached the quest goal."))
                        .show();
                }

                if auto_stop {
                    let state = app.state::<AppState>();
                    let dummy = state
                        .running
                        .lock()
                        .ok()
                        .and_then(|mut running| running.remove(&key));
                    if let Some(dummy) = dummy {
                        stop_running_dummy(dummy);
                        let _ = app.emit(
                            "dummy://exited",
                            ExitedPayload {
                                key: key.clone(),
                                name: name.clone(),
                                code: None,
                            },
                        );
                        update_tray_menu(&app, state.inner());
                    }
                }
                break;
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    stop
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn send_dummy_command(state: &AppState, key: &str, command: &str) {
    if let Ok(mut running) = state.running.lock()
        && let Some(dummy) = running.get_mut(key)
    {
        let _ = dummy.stdin.write_all(command.as_bytes());
        let _ = dummy.stdin.flush();
    }
}

fn update_tray_menu(app: &AppHandle, state: &AppState) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };

    let entries: Vec<(String, String)> = {
        let Ok(running) = state.running.lock() else {
            return;
        };
        running
            .iter()
            .map(|(key, dummy)| (key.clone(), dummy.name.clone()))
            .collect()
    };

    let build = || -> tauri::Result<()> {
        let menu = Menu::new(app)?;
        let open = MenuItem::with_id(app, "open", "Open Dummies", true, None::<&str>)?;
        menu.append(&open)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;

        if entries.is_empty() {
            let empty = MenuItem::with_id(app, "empty", "No games running", false, None::<&str>)?;
            menu.append(&empty)?;
        } else {
            for (key, name) in &entries {
                let submenu = Submenu::with_id(app, format!("game:{key}"), name, true)?;
                let show = MenuItem::with_id(
                    app,
                    format!("show:{key}"),
                    "Show window",
                    true,
                    None::<&str>,
                )?;
                let hide = MenuItem::with_id(
                    app,
                    format!("hide:{key}"),
                    "Hide window",
                    true,
                    None::<&str>,
                )?;
                let stop =
                    MenuItem::with_id(app, format!("stop:{key}"), "Stop game", true, None::<&str>)?;
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
            "Stop all games",
            !entries.is_empty(),
            None::<&str>,
        )?;
        menu.append(&stop_all)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        menu.append(&quit)?;

        tray.set_menu(Some(menu))?;
        let tooltip = if entries.is_empty() {
            "Dummies".to_string()
        } else {
            format!("Dummies — {} running", entries.len())
        };
        tray.set_tooltip(Some(tooltip))?;
        Ok(())
    };

    let _ = build();
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

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id("main")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_menu(app, event.id().as_ref()))
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
            let state = app.state::<AppState>();
            update_tray_menu(&app, state.inner());
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
    let mut library = load_library(&state.paths.my_games_path);
    {
        let database = state.database.read().map_err(|error| error.to_string())?;
        if enrich_library(&mut library, &database) {
            let _ = write_library(&state.paths.my_games_path, &library);
        }
    }

    Ok(Bootstrap {
        settings,
        system_locale: get_system_locale(),
        running_keys: running_keys(&state)?,
        library,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tauri::command]
async fn sync_gamelist(state: State<'_, AppState>) -> Result<SyncResult, String> {
    let path = state.paths.gamelist_path.clone();
    let fetch_path = path.clone();
    let updated = tauri::async_runtime::spawn_blocking(move || fetch_gamelist(&fetch_path))
        .await
        .map_err(|error| error.to_string())??;
    let database = load_database(&path);
    let count = database.len();
    *state.database.write().map_err(|error| error.to_string())? = database;

    Ok(SyncResult {
        updated,
        count,
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn get_database_page(
    state: State<AppState>,
    filter: String,
    offset: usize,
    limit: usize,
) -> Result<DatabasePage, String> {
    let database = state.database.read().map_err(|error| error.to_string())?;
    Ok(page_database(&database, &filter, offset, limit))
}

#[tauri::command]
fn get_library(state: State<AppState>) -> Result<Vec<LibraryGame>, String> {
    let mut library = load_library(&state.paths.my_games_path);
    {
        let database = state.database.read().map_err(|error| error.to_string())?;
        if enrich_library(&mut library, &database) {
            let _ = write_library(&state.paths.my_games_path, &library);
        }
    }
    Ok(library)
}

#[tauri::command]
fn add_game(state: State<AppState>, request: AddGameRequest) -> Result<LibraryGame, String> {
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

    let mut library = load_library(&state.paths.my_games_path);
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

#[tauri::command]
fn remove_game(state: State<AppState>, app_id: String, exe: String) -> Result<bool, String> {
    let mut library = load_library(&state.paths.my_games_path);
    let candidate = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .cloned();
    let before = library.len();
    library.retain(|game| !(game.app_id == app_id && game.exe == exe));
    let removed = library.len() != before;

    if removed {
        if let Some(game) = candidate
            && game.use_steam_path
            && let Some(steam_path) = find_steam_path()
        {
            let steam_apps = steam_path.join("steamapps");
            if let Some(steam_app_id) = game.steam_app_id.as_deref() {
                let _ = fs::remove_file(steam_apps.join(format!("appmanifest_{steam_app_id}.acf")));
            }
            if let Some(install_dir) = game.install_dir.as_deref() {
                let _ = fs::remove_dir_all(steam_apps.join("common").join(install_dir));
            }
        }

        write_library(&state.paths.my_games_path, &library)?;
    }

    Ok(removed)
}

#[tauri::command]
fn toggle_favorite(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<LibraryGame, String> {
    let mut library = load_library(&state.paths.my_games_path);
    let game = library
        .iter_mut()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    game.is_favorite = !game.is_favorite;
    let updated = game.clone();
    write_library(&state.paths.my_games_path, &library)?;
    Ok(updated)
}

#[tauri::command]
fn update_game_executable(
    state: State<AppState>,
    app_id: String,
    old_exe: String,
    new_exe: String,
) -> Result<LibraryGame, String> {
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

    let mut library = load_library(&state.paths.my_games_path);
    let game = library
        .iter_mut()
        .find(|game| game.app_id == app_id && game.exe == old_exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    game.exe = new_exe;
    let updated = game.clone();
    write_library(&state.paths.my_games_path, &library)?;
    Ok(updated)
}

fn find_steam_path() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam")
        && let Ok(value) = key.get_value::<String, _>("SteamPath")
    {
        let path = PathBuf::from(value.replace('/', "\\"));
        if path.is_dir() {
            return Some(path);
        }
    }

    [
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ]
    .into_iter()
    .find(|path| path.is_dir())
}

#[tauri::command]
fn get_steam_path() -> Option<String> {
    find_steam_path().map(|path| path.to_string_lossy().to_string())
}

fn parse_version(value: &str) -> Option<Vec<u32>> {
    let trimmed = value.trim().trim_start_matches(['v', 'V']);
    let core = trimmed.split(['-', '+']).next()?;
    core.split('.')
        .map(|part| part.parse::<u32>().ok())
        .collect()
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let Some(latest) = parse_version(latest) else {
        return false;
    };
    let Some(current) = parse_version(current) else {
        return false;
    };

    for index in 0..3 {
        let next = *latest.get(index).unwrap_or(&0);
        let installed = *current.get(index).unwrap_or(&0);
        if next != installed {
            return next > installed;
        }
    }

    false
}

#[tauri::command]
async fn check_for_updates() -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Dummies/1.0")
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .get("https://api.github.com/repos/Morphy137/discord-dummies-launcher/releases/latest")
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|error| error.to_string())?;

        if !response.status().is_success() {
            return Err(format!("GitHub returned {}", response.status()));
        }

        let json: serde_json::Value = response.json().map_err(|error| error.to_string())?;
        let latest_version = json
            .get("tag_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .trim_start_matches(['v', 'V'])
            .to_string();
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        Ok(UpdateInfo {
            update_available: version_is_newer(&latest_version, &current_version),
            current_version,
            latest_version,
            release_url: json
                .get("html_url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            release_name: json
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            release_notes: json
                .get("body")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn setup_steam_integration(
    app: AppHandle,
    state: State<AppState>,
    app_id: String,
    steam_app_id: String,
    install_dir: String,
    exe: String,
) -> Result<SteamSetupResult, String> {
    let steam_id = steam_app_id.trim().to_string();
    if steam_id.is_empty() || !steam_id.chars().all(|character| character.is_ascii_digit()) {
        return Err("A numeric Steam App ID is required".to_string());
    }

    let install =
        sanitize_component(&install_dir).ok_or_else(|| "Invalid install folder".to_string())?;
    let exe_relative =
        sanitize_relative_exe(&exe).ok_or_else(|| "Invalid executable path".to_string())?;
    let exe_name = exe_relative
        .file_name()
        .ok_or_else(|| "Invalid executable name".to_string())?;
    let exe_parent = exe_relative.parent().unwrap_or_else(|| Path::new(""));
    let steam_path = find_steam_path()
        .ok_or_else(|| "Steam installation path could not be found".to_string())?;
    let source = find_dummy_source(&app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;

    let steam_apps = steam_path.join("steamapps");
    fs::create_dir_all(&steam_apps).map_err(|error| error.to_string())?;
    let manifest_path = steam_apps.join(format!("appmanifest_{steam_id}.acf"));
    let target_dir = steam_apps.join("common").join(&install).join(exe_parent);
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;
    let destination = target_dir.join(exe_name);

    let mut library = load_library(&state.paths.my_games_path);
    let game = library
        .iter_mut()
        .find(|game| game.app_id == app_id)
        .ok_or_else(|| "Game not found in library".to_string())?;
    let game_name = game.name.clone();

    let manifest = format!(
        "\"AppState\"\n{{\n    \"appid\" \"{steam_id}\"\n    \"name\" \"{game_name}\"\n    \"installdir\" \"{install}\"\n}}\n"
    );
    fs::write(&manifest_path, manifest).map_err(|error| error.to_string())?;
    fs::copy(&source, &destination).map_err(|error| error.to_string())?;

    game.use_steam_path = true;
    game.steam_app_id = Some(steam_id);
    game.install_dir = Some(install);
    game.exe = exe_relative.to_string_lossy().to_string();
    game.steam_exe_path = Some(destination.to_string_lossy().to_string());
    let updated_game = game.clone();
    write_library(&state.paths.my_games_path, &library)?;

    Ok(SteamSetupResult {
        manifest_path: manifest_path.to_string_lossy().to_string(),
        dest_exe_path: destination.to_string_lossy().to_string(),
        steam_path: steam_path.to_string_lossy().to_string(),
        updated_game,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamRemoveResult {
    removed_manifest: bool,
    removed_folder: bool,
    updated_game: Option<LibraryGame>,
}

#[tauri::command]
fn remove_steam_integration(
    state: State<AppState>,
    app_id: String,
    steam_app_id: Option<String>,
    install_dir: Option<String>,
) -> Result<SteamRemoveResult, String> {
    let steam_path = find_steam_path()
        .ok_or_else(|| "Steam installation path could not be found".to_string())?;
    let steam_apps = steam_path.join("steamapps");
    let mut library = load_library(&state.paths.my_games_path);
    let game = library.iter_mut().find(|game| game.app_id == app_id);
    let effective_app_id = steam_app_id
        .or_else(|| game.as_ref().and_then(|game| game.steam_app_id.clone()))
        .unwrap_or_default();
    let effective_install_dir = install_dir
        .or_else(|| game.as_ref().and_then(|game| game.install_dir.clone()))
        .unwrap_or_default();

    let mut removed_manifest = false;
    let mut removed_folder = false;

    if !effective_app_id.is_empty() {
        let manifest = steam_apps.join(format!("appmanifest_{effective_app_id}.acf"));
        if manifest.is_file() {
            fs::remove_file(&manifest).map_err(|error| error.to_string())?;
            removed_manifest = true;
        }
    }

    if !effective_install_dir.is_empty() {
        let folder = steam_apps.join("common").join(&effective_install_dir);
        if folder.is_dir() {
            fs::remove_dir_all(&folder).map_err(|error| error.to_string())?;
            removed_folder = true;
        }
    }

    let updated_game = if let Some(game) = game {
        game.use_steam_path = false;
        game.steam_exe_path = None;
        let updated = game.clone();
        write_library(&state.paths.my_games_path, &library)?;
        Some(updated)
    } else {
        None
    };

    Ok(SteamRemoveResult {
        removed_manifest,
        removed_folder,
        updated_game,
    })
}

#[tauri::command]
fn create_shortcut(
    app: AppHandle,
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<String, String> {
    let library = load_library(&state.paths.my_games_path);
    let game = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    let source = find_dummy_source(&app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    let (destination, working_dir) = prepare_dummy(&source, &state.paths, &game.app_id, &game.exe)?;
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
    link.set_arguments(Some(format!("\"{}\"", game.name)));
    link.set_working_dir(Some(working_dir.to_string_lossy().to_string()));
    link.create_lnk(&shortcut_path)
        .map_err(|error| error.to_string())?;

    Ok(shortcut_path.to_string_lossy().to_string())
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

    let timer_stop = if settings.quest_timer_enabled {
        Some(start_quest_timer(
            app.clone(),
            game_key.clone(),
            name.clone(),
            settings.quest_duration_minutes,
            settings.quest_sync_buffer_seconds,
            settings.auto_stop_on_complete,
            settings.notify_on_complete,
        ))
    } else {
        None
    };

    let dummy = RunningDummy {
        child,
        stdin,
        name,
        timer_stop,
    };

    state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .insert(game_key.clone(), dummy);

    update_tray_menu(&app, state.inner());

    Ok(LaunchResult {
        game_key,
        exe_path: destination.to_string_lossy().to_string(),
        already_running: false,
    })
}

#[tauri::command]
fn stop_dummy(app: AppHandle, state: State<AppState>, game_key: String) -> Result<bool, String> {
    let dummy = state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&game_key);

    if let Some(dummy) = dummy {
        stop_running_dummy(dummy);
        update_tray_menu(&app, state.inner());
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn stop_all_dummies(app: AppHandle, state: State<AppState>) -> Result<usize, String> {
    let mut running = state.running.lock().map_err(|error| error.to_string())?;
    let count = running.len();
    for (_, dummy) in running.drain() {
        stop_running_dummy(dummy);
    }
    drop(running);
    update_tray_menu(&app, state.inner());
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
            app.handle().plugin(tauri_plugin_notification::init())?;
            app.handle().plugin(tauri_plugin_opener::init())?;

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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_system_locale,
            get_bootstrap,
            sync_gamelist,
            get_database_page,
            get_library,
            add_game,
            remove_game,
            toggle_favorite,
            update_game_executable,
            create_shortcut,
            get_steam_path,
            setup_steam_integration,
            remove_steam_integration,
            check_for_updates,
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
            quest_timer_enabled: true,
            quest_duration_minutes: 0,
            quest_sync_buffer_seconds: 999,
            auto_stop_on_complete: true,
            notify_on_complete: true,
        });

        assert_eq!(settings.theme, "system");
        assert_eq!(settings.language, "system");
        assert_eq!(settings.cards_per_row, 6);
        assert_eq!(settings.quest_duration_minutes, 15);
        assert_eq!(settings.quest_sync_buffer_seconds, 60);
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
            quest_timer_enabled: true,
            quest_duration_minutes: 20,
            quest_sync_buffer_seconds: 30,
            auto_stop_on_complete: false,
            notify_on_complete: false,
        };

        write_settings(&path, &settings).expect("write settings");
        let loaded = load_settings(&path);

        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "es");
        assert_eq!(loaded.cards_per_row, 7);
        assert_eq!(loaded.quest_duration_minutes, 20);
        assert_eq!(loaded.quest_sync_buffer_seconds, 30);
        assert!(!loaded.auto_stop_on_complete);
        assert!(!loaded.notify_on_complete);
        assert!(loaded.show_dummy_window);
        assert!(!loaded.minimize_to_tray);

        fs::remove_dir_all(directory).expect("cleanup temp dir");
    }

    #[test]
    fn builds_database_games_and_prefers_non_launcher_win32() {
        let games = to_database_games(vec![DetectableApp {
            id: Some("123".to_string()),
            name: Some("Example Game".to_string()),
            executables: Some(vec![
                AppExecutable {
                    name: Some("launcher.exe".to_string()),
                    os: Some("win32".to_string()),
                    is_launcher: Some(true),
                },
                AppExecutable {
                    name: Some("game.exe".to_string()),
                    os: Some("win32".to_string()),
                    is_launcher: Some(false),
                },
            ]),
            icon_hash: Some("icon".to_string()),
            cover_image_hash: Some("cover".to_string()),
            third_party_skus: Some(vec![ThirdPartySku {
                distributor: Some("steam".to_string()),
                id: Some("456".to_string()),
            }]),
        }]);

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].exe, "game.exe");
        assert_eq!(games[0].steam_app_id.as_deref(), Some("456"));
        assert!(!games[0].uses_new_detection);
    }

    #[test]
    fn pages_and_filters_database_games() {
        let database = to_database_games(vec![
            DetectableApp {
                id: Some("1".to_string()),
                name: Some("Alpha".to_string()),
                executables: Some(vec![AppExecutable {
                    name: Some("alpha.exe".to_string()),
                    os: Some("win32".to_string()),
                    is_launcher: Some(false),
                }]),
                icon_hash: None,
                cover_image_hash: None,
                third_party_skus: None,
            },
            DetectableApp {
                id: Some("2".to_string()),
                name: Some("Beta".to_string()),
                executables: Some(vec![AppExecutable {
                    name: Some("beta.exe".to_string()),
                    os: Some("win32".to_string()),
                    is_launcher: Some(false),
                }]),
                icon_hash: None,
                cover_image_hash: None,
                third_party_skus: None,
            },
        ]);

        let page = page_database(&database, "a", 0, 1);
        assert_eq!(page.items.len(), 1);
        assert!(page.has_more);
        assert_eq!(page.items[0].name, "Alpha");
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
            my_games_path: directory.join("myGames.json"),
            gamelist_path: directory.join("gamelist.json"),
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
