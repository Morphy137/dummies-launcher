use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

const DISCORD_DETECTABLE_URL: &str = "https://discord.com/api/applications/detectable";

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
    let before = library.len();
    library.retain(|game| !(game.app_id == app_id && game.exe == exe));
    let removed = library.len() != before;

    if removed {
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
