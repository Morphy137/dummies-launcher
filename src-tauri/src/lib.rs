use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_notification::NotificationExt;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

#[cfg(test)]
mod regression_tests;
mod storage;

static DATA_OPERATIONS: Mutex<()> = Mutex::new(());
static CATALOG_SYNC: Mutex<()> = Mutex::new(());
static UPDATING: AtomicBool = AtomicBool::new(false);

const DISCORD_DETECTABLE_URL: &str = "https://discord.com/api/applications/detectable";

static NEXT_TIMER_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    schema_version: u32,
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
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: "system".to_string(),
            language: "system".to_string(),
            show_dummy_window: true,
            minimize_to_tray: true,
            cards_per_row: 6,
            quest_timer_enabled: true,
            quest_duration_minutes: 15,
            quest_sync_buffer_seconds: 60,
            auto_stop_on_complete: true,
            notify_on_complete: true,
            extra: serde_json::Map::new(),
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
    portable: bool,
    quest_ticks: Vec<QuestTick>,
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
    steam_library_path: Option<String>,
    steam_previous_exe: Option<String>,
    steam_manifest_sha256: Option<String>,
    steam_dummy_sha256: Option<String>,
    steam_created_install_dir: bool,
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
            steam_library_path: None,
            steam_previous_exe: None,
            steam_manifest_sha256: None,
            steam_dummy_sha256: None,
            steam_created_install_dir: false,
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
struct SteamSetupResult {
    manifest_path: String,
    dest_exe_path: String,
    steam_path: String,
    updated_game: LibraryGame,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SteamSetupRequest {
    app_id: String,
    current_exe: String,
    steam_app_id: String,
    install_dir: String,
    exe: String,
    library_path: Option<String>,
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
    quest_clock: Option<Arc<Mutex<QuestClock>>>,
    last_tick: Option<Arc<Mutex<Option<QuestTick>>>>,
}

impl Drop for RunningDummy {
    fn drop(&mut self) {
        if let Some(stop) = &self.timer_stop {
            stop.store(true, Ordering::SeqCst);
        }
    }
}

struct QuestClock {
    started: Instant,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl QuestClock {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            paused_at: None,
            paused_total: Duration::ZERO,
        }
    }

    fn elapsed(&self) -> Duration {
        let now = Instant::now();
        let paused_now = self
            .paused_at
            .map(|paused_at| now.saturating_duration_since(paused_at))
            .unwrap_or_default();
        now.saturating_duration_since(self.started)
            .saturating_sub(self.paused_total + paused_now)
    }

    fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    fn pause(&mut self) -> bool {
        if self.paused_at.is_some() {
            return false;
        }
        self.paused_at = Some(Instant::now());
        true
    }

    fn resume(&mut self) -> bool {
        let Some(paused_at) = self.paused_at.take() else {
            return false;
        };
        self.paused_total += paused_at.elapsed();
        true
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestTick {
    key: String,
    name: String,
    timer_id: u64,
    remaining_seconds: u64,
    phase: String,
    percent: u8,
    paused: bool,
}

struct AppState {
    paths: AppPaths,
    settings: Mutex<Settings>,
    database: RwLock<Vec<DatabaseGame>>,
    running: Arc<Mutex<HashMap<String, RunningDummy>>>,
}

fn load_settings(path: &Path) -> Settings {
    match storage::read_json::<Settings>(path) {
        Ok(settings) => normalize_settings(settings),
        Err(error) => {
            log::error!("{error}");
            Settings::default()
        }
    }
}

fn normalize_settings(mut settings: Settings) -> Settings {
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

fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    storage::write_json(path, settings)
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
    validate_catalog(remote)?;
    let local = fs::read_to_string(path)
        .ok()
        .map(|content| content.trim().to_string());

    if local.as_deref() == Some(remote) {
        return Ok(false);
    }

    storage::write_atomic(path, format!("{remote}\n").as_bytes(), true)?;
    Ok(true)
}

fn validate_catalog(raw: &str) -> Result<(), String> {
    let apps: Vec<DetectableApp> = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if apps.is_empty()
        || apps.iter().any(|app| {
            app.id.as_deref().is_none_or(str::is_empty)
                || app.name.as_deref().is_none_or(str::is_empty)
        })
    {
        return Err(
            "The downloaded catalog is incomplete; the cached catalog was preserved".into(),
        );
    }
    Ok(())
}

fn load_library(path: &Path) -> Result<Vec<LibraryGame>, String> {
    let library: Vec<LibraryGame> = storage::read_json(path)?;
    validate_library(&library)?;
    Ok(library)
}

fn validate_library(library: &[LibraryGame]) -> Result<(), String> {
    let mut keys = std::collections::HashSet::new();
    for game in library {
        let exe = sanitize_relative_exe(&game.exe)
            .ok_or_else(|| "Invalid executable in library; restore its .bak file".to_string())?;
        if !safe_path_component(&game.app_id)
            || game.name.trim().is_empty()
            || !keys.insert(format!(
                "{}::{}",
                game.app_id.to_lowercase(),
                exe.to_string_lossy().to_lowercase()
            ))
        {
            return Err("Invalid or duplicate library entry; restore its .bak file".into());
        }
    }
    Ok(())
}

fn write_library(path: &Path, library: &[LibraryGame]) -> Result<(), String> {
    validate_library(library)?;
    storage::write_json(path, library)
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
    migrate_legacy_from(&base, paths);
}

fn migrate_legacy_from(base: &Path, paths: &AppPaths) {
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

        for (name, destination) in files {
            if destination.exists() {
                continue;
            }
            let source = directory.join(name);
            if source.is_file() {
                match fs::read(&source) {
                    Ok(bytes) if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() => {
                        if let Err(error) = storage::write_atomic(destination, &bytes, false)
                            .and_then(|_| {
                                storage::write_atomic(
                                    &destination.with_extension("json.bak"),
                                    &bytes,
                                    false,
                                )
                            })
                        {
                            log::error!("Migration failed: {error}");
                        }
                    }
                    _ => log::warn!("Skipping unreadable legacy file: {}", source.display()),
                }
            }
        }
    }
}

fn migrate_dummy_window_visibility(paths: &AppPaths) {
    let marker = paths
        .settings_path
        .with_file_name(".dummy-window-visible-v1");
    if marker.exists() {
        return;
    }

    if paths.settings_path.is_file() {
        let mut settings = load_settings(&paths.settings_path);
        if !settings.show_dummy_window {
            settings.show_dummy_window = true;
            if write_settings(&paths.settings_path, &settings).is_err() {
                return;
            }
        }
    }

    let _ = fs::write(marker, b"1");
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
    if normalized.starts_with('/') || normalized.contains(':') {
        return None;
    }
    let mut path = PathBuf::new();

    for component in normalized.split('/') {
        if !safe_path_component(component) {
            return None;
        }
        path.push(component);
    }

    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn safe_path_component(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 240
        && !value.ends_with(['.', ' '])
        && !value
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
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
    if !safe_path_component(app_id) {
        return Err("Invalid app id".into());
    }
    let app_folder = app_id;
    let exe_relative =
        sanitize_relative_exe(exe).ok_or_else(|| "Invalid executable path".to_string())?;
    let exe_name = exe_relative
        .file_name()
        .ok_or_else(|| "Invalid executable name".to_string())?;
    let exe_parent = exe_relative.parent().unwrap_or_else(|| Path::new(""));
    let working_dir = paths.games_dir.join(app_folder).join(exe_parent);
    let destination = working_dir.join(exe_name);

    reject_redirects(&working_dir)?;
    fs::create_dir_all(&working_dir).map_err(|error| error.to_string())?;

    ensure_existing_within(&paths.games_dir, &working_dir)?;
    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    if destination.exists() {
        ensure_existing_within(&working_dir, &destination)?;
    }
    if fs::read(&destination).ok().as_deref() != Some(bytes.as_slice()) {
        storage::write_atomic(&destination, &bytes, false)?;
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

struct QuestTimerConfig {
    target_minutes: u32,
    buffer_seconds: u32,
    auto_stop: bool,
    notify: bool,
    spanish: bool,
}

struct QuestTimerHandles {
    stop: Arc<AtomicBool>,
    clock: Arc<Mutex<QuestClock>>,
    last_tick: Arc<Mutex<Option<QuestTick>>>,
}

fn start_quest_timer(
    app: AppHandle,
    key: String,
    name: String,
    timer_id: u64,
    config: QuestTimerConfig,
) -> QuestTimerHandles {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_signal = stop.clone();
    let clock = Arc::new(Mutex::new(QuestClock::new()));
    let clock_signal = clock.clone();
    let last_tick = Arc::new(Mutex::new(None));
    let tick_signal = last_tick.clone();

    thread::spawn(move || {
        let target = u64::from(config.target_minutes) * 60;
        let total = target + u64::from(config.buffer_seconds);

        loop {
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }

            let (elapsed, paused) = {
                let clock = clock_signal
                    .lock()
                    .expect("quest clock should not be poisoned");
                (clock.elapsed(), clock.is_paused())
            };
            let elapsed = elapsed.as_secs();
            let remaining = total.saturating_sub(elapsed);
            let phase = if elapsed >= total {
                "complete"
            } else if elapsed >= target {
                "syncing"
            } else {
                "playing"
            };
            let percent = (elapsed.min(total) * 100).checked_div(total).unwrap_or(100) as u8;

            let state = app.state::<AppState>();
            let active = state
                .running
                .lock()
                .map(|running| {
                    running
                        .get(&key)
                        .and_then(|dummy| dummy.quest_clock.as_ref())
                        .is_some_and(|current| Arc::ptr_eq(current, &clock_signal))
                })
                .unwrap_or(false);
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }
            if !active {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let tick = QuestTick {
                key: key.clone(),
                name: name.clone(),
                timer_id,
                remaining_seconds: remaining,
                phase: phase.to_string(),
                percent,
                paused,
            };
            if let Ok(mut current) = tick_signal.lock() {
                *current = Some(tick.clone());
            }
            let _ = app.emit("quest://tick", tick);

            if phase == "complete" {
                if config.notify {
                    let _ = app
                        .notification()
                        .builder()
                        .title(if config.spanish {
                            "Temporizador completado"
                        } else {
                            "Timer complete"
                        })
                        .body(if config.spanish {
                            format!("{name} alcanzó el tiempo objetivo local.")
                        } else {
                            format!("{name} reached the local timer goal.")
                        })
                        .show();
                }

                if config.auto_stop {
                    let state = app.state::<AppState>();
                    let dummy = state.running.lock().ok().and_then(|mut running| {
                        if running
                            .get(&key)
                            .and_then(|dummy| dummy.quest_clock.as_ref())
                            .is_some_and(|current| Arc::ptr_eq(current, &clock_signal))
                        {
                            running.remove(&key)
                        } else {
                            None
                        }
                    });
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

    QuestTimerHandles {
        stop,
        clock,
        last_tick,
    }
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

fn is_spanish(settings: &Settings) -> bool {
    settings.language == "es"
        || (settings.language == "system" && get_system_locale().to_lowercase().starts_with("es"))
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

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let normalized = normalize_settings(settings);
    write_settings(&state.paths.settings_path, &normalized)?;
    *state.settings.lock().map_err(|error| error.to_string())? = normalized.clone();
    update_tray_menu(&app, state.inner());
    Ok(normalized)
}

#[tauri::command]
fn get_system_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_string())
}

#[tauri::command(async)]
fn get_bootstrap(state: State<AppState>) -> Result<Bootstrap, String> {
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
        running_keys: running_keys(&state)?,
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

#[tauri::command(async)]
fn sync_gamelist(state: State<'_, AppState>) -> Result<SyncResult, String> {
    let _guard = CATALOG_SYNC.lock().map_err(|error| error.to_string())?;
    let path = state.paths.gamelist_path.clone();
    let updated = fetch_gamelist(&path)?;
    let database = load_database(&path);
    let count = database.len();
    *state.database.write().map_err(|error| error.to_string())? = database;

    Ok(SyncResult {
        updated,
        count,
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command(async)]
fn get_database_page(
    state: State<AppState>,
    filter: String,
    offset: usize,
    limit: usize,
) -> Result<DatabasePage, String> {
    let database = state.database.read().map_err(|error| error.to_string())?;
    Ok(page_database(&database, &filter, offset, limit))
}

#[tauri::command(async)]
fn get_library(state: State<AppState>) -> Result<Vec<LibraryGame>, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    {
        let database = state.database.read().map_err(|error| error.to_string())?;
        if enrich_library(&mut library, &database) {
            write_library(&state.paths.my_games_path, &library)?;
        }
    }
    Ok(library)
}

#[tauri::command(async)]
fn add_game(state: State<AppState>, request: AddGameRequest) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
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

    let mut library = load_library(&state.paths.my_games_path)?;
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

#[tauri::command(async)]
fn remove_game(state: State<AppState>, app_id: String, exe: String) -> Result<bool, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    let candidate = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .cloned();
    if let Some(game) = candidate.as_ref() {
        let running_key = format!("{}::{}", game.app_id, game.exe);
        if state
            .running
            .lock()
            .map_err(|error| error.to_string())?
            .contains_key(&running_key)
        {
            return Err("Stop the game before removing it from the library".to_string());
        }
        if game.use_steam_path {
            return Err(
                "Remove the Steam integration before removing this game from the library"
                    .to_string(),
            );
        }
    }

    let before = library.len();
    library.retain(|game| !(game.app_id == app_id && game.exe == exe));
    let removed = library.len() != before;

    if removed {
        write_library(&state.paths.my_games_path, &library)?;
    }

    Ok(removed)
}

#[tauri::command(async)]
fn toggle_favorite(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let mut library = load_library(&state.paths.my_games_path)?;
    let game = library
        .iter_mut()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    game.is_favorite = !game.is_favorite;
    let updated = game.clone();
    write_library(&state.paths.my_games_path, &library)?;
    Ok(updated)
}

#[tauri::command(async)]
fn update_game_executable(
    state: State<AppState>,
    app_id: String,
    old_exe: String,
    new_exe: String,
) -> Result<LibraryGame, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
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

    let mut library = load_library(&state.paths.my_games_path)?;
    let index = library
        .iter()
        .position(|game| game.app_id == app_id && game.exe == old_exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    if library[index].use_steam_path {
        return Err("Remove the Steam integration before changing the game executable".to_string());
    }
    if new_exe == old_exe {
        return Ok(library[index].clone());
    }
    if library
        .iter()
        .any(|game| game.app_id == app_id && game.exe.eq_ignore_ascii_case(&new_exe))
    {
        return Err("Another library entry already uses that executable".to_string());
    }

    library[index].exe = new_exe;
    let updated = library[index].clone();
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

fn vdf_strings(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = raw.chars();
    while let Some(character) = chars.next() {
        if character != '"' {
            continue;
        }
        let mut token = String::new();
        while let Some(character) = chars.next() {
            match character {
                '"' => break,
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        token.push(escaped);
                    }
                }
                _ => token.push(character),
            }
        }
        tokens.push(token);
    }
    tokens
}

fn steam_libraries() -> Vec<PathBuf> {
    let Some(primary) = find_steam_path() else {
        return Vec::new();
    };
    let mut paths = vec![primary.clone()];
    if let Ok(raw) = fs::read_to_string(primary.join("steamapps/libraryfolders.vdf")) {
        let tokens = vdf_strings(&raw);
        for pair in tokens.windows(2) {
            if pair[0] == "path" {
                let path = PathBuf::from(&pair[1]);
                if path.is_absolute() && path.is_dir() && !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

#[tauri::command(async)]
fn get_steam_libraries() -> Vec<String> {
    steam_libraries()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

fn checked_steam_library(requested: Option<&str>) -> Result<PathBuf, String> {
    let paths = steam_libraries();
    let path = if let Some(requested) = requested {
        let requested = fs::canonicalize(requested).map_err(|error| error.to_string())?;
        paths
            .into_iter()
            .find(|path| fs::canonicalize(path).ok().as_ref() == Some(&requested))
    } else {
        paths.into_iter().next()
    };
    let path =
        path.ok_or_else(|| "Steam library was not found in Steam configuration".to_string())?;
    reject_redirects(&path)?;
    Ok(path)
}

#[derive(Clone, Debug)]
struct SteamLayout {
    steam_id: String,
    install_dir: String,
    exe_relative: PathBuf,
    steam_apps: PathBuf,
    common: PathBuf,
    manifest_path: PathBuf,
    install_root: PathBuf,
    destination: PathBuf,
}

fn steam_layout(
    steam_path: &Path,
    steam_app_id: &str,
    install_dir: &str,
    exe: &str,
) -> Result<SteamLayout, String> {
    let steam_id = steam_app_id.trim().to_string();
    if steam_id.is_empty() || !steam_id.chars().all(|character| character.is_ascii_digit()) {
        return Err("A numeric Steam App ID is required".to_string());
    }

    if !safe_path_component(install_dir) {
        return Err("Invalid install folder".into());
    }
    let install = install_dir.to_string();
    let mut exe_relative =
        sanitize_relative_exe(exe).ok_or_else(|| "Invalid executable path".to_string())?;
    if exe_relative
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
    {
        exe_relative.set_extension("exe");
    }

    let exe_name = exe_relative
        .file_name()
        .ok_or_else(|| "Invalid executable name".to_string())?;
    let exe_parent = exe_relative.parent().unwrap_or_else(|| Path::new(""));
    let steam_apps = steam_path.join("steamapps");
    let common = steam_apps.join("common");
    let install_root = common.join(&install);
    let destination = install_root.join(exe_parent).join(exe_name);

    Ok(SteamLayout {
        steam_id: steam_id.clone(),
        install_dir: install,
        exe_relative,
        manifest_path: steam_apps.join(format!("appmanifest_{steam_id}.acf")),
        steam_apps,
        common,
        install_root,
        destination,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn steam_manifest(game_name: &str, steam_id: &str, install_dir: &str) -> String {
    let escaped_name: String = game_name
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "\"AppState\"\n{{\n    \"appid\" \"{steam_id}\"\n    \"name\" \"{escaped_name}\"\n    \"installdir\" \"{install_dir}\"\n}}\n"
    )
}

fn reject_redirects(path: &Path) -> Result<(), String> {
    // Refuse junctions and symlinks, even when their target is currently inside base.
    let mut current = Some(path);
    while let Some(part) = current {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            match fs::symlink_metadata(part) {
                Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => {
                    return Err("Refusing a redirected filesystem path".into());
                }
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.to_string());
                }
                _ => {}
            }
        }
        current = part.parent();
    }
    Ok(())
}

fn ensure_existing_within(base: &Path, path: &Path) -> Result<(), String> {
    reject_redirects(path)?;
    let canonical_base = fs::canonicalize(base).map_err(|error| error.to_string())?;
    let canonical_path = fs::canonicalize(path).map_err(|error| error.to_string())?;
    if canonical_path.starts_with(canonical_base) {
        Ok(())
    } else {
        Err(format!(
            "Refusing to access a path outside the expected folder: {}",
            path.display()
        ))
    }
}

fn read_owned_artifact(
    base: &Path,
    path: &Path,
    expected_sha256: Option<&str>,
) -> Result<Option<Vec<u8>>, String> {
    if !path.exists() {
        return Ok(None);
    }

    ensure_existing_within(base, path)?;
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let owned = if let Some(expected) = expected_sha256 {
        sha256_hex(&bytes) == expected
    } else {
        false
    };

    if !owned {
        return Err(format!(
            "Dummies cannot verify ownership of {}. The file was left unchanged.",
            path.display()
        ));
    }

    Ok(Some(bytes))
}

fn write_new_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())
    })();
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

fn write_artifact(path: &Path, bytes: &[u8], allow_replace: bool) -> Result<(), String> {
    if !path.exists() {
        return write_new_artifact(path, bytes);
    }
    if !allow_replace {
        return Err(format!(
            "Refusing to overwrite an existing file: {}",
            path.display()
        ));
    }

    let parent = path
        .parent()
        .ok_or_else(|| "Artifact path has no parent".to_string())?;
    let temporary = parent.join(format!(
        ".dummies-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    write_new_artifact(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

fn restore_artifact(path: &Path, previous: Option<&[u8]>, written: Option<&[u8]>) {
    let result = (|| -> Result<(), String> {
        if let Some(written) = written {
            if fs::read(path).map_err(|error| error.to_string())? != written {
                return Err("File changed externally during rollback; preserving it".into());
            }
        } else if path.exists() {
            return Err("A new file appeared during rollback; preserving it".into());
        }
        if let Some(bytes) = previous {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            write_artifact(path, bytes, written.is_some())?;
        } else if written.is_some() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        log::error!(
            "Rollback requires manual inspection of {}: {error}",
            path.display()
        );
    }
}

fn remove_empty_steam_directories(layout: &SteamLayout) -> bool {
    let mut current = layout.destination.parent();
    while let Some(directory) = current {
        if directory == layout.install_root {
            break;
        }
        if fs::remove_dir(directory).is_err() {
            return false;
        }
        current = directory.parent();
    }

    fs::remove_dir(&layout.install_root).is_ok()
}

fn setup_steam_at(
    state: &AppState,
    source: &Path,
    steam_path: &Path,
    request: &SteamSetupRequest,
) -> Result<SteamSetupResult, String> {
    let app_id = request.app_id.as_str();
    let current_exe = request.current_exe.as_str();
    let steam_app_id = request.steam_app_id.as_str();
    let install_dir = request.install_dir.as_str();
    let exe = request.exe.as_str();
    let layout = steam_layout(steam_path, steam_app_id, install_dir, exe)?;
    reject_redirects(&layout.destination)?;
    reject_redirects(&layout.manifest_path)?;
    let new_exe = layout.exe_relative.to_string_lossy().to_string();
    let running_key = format!("{app_id}::{current_exe}");
    if state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .contains_key(&running_key)
    {
        return Err("Stop the game before changing its Steam integration".to_string());
    }

    let mut library = load_library(&state.paths.my_games_path)?;
    let index = library
        .iter()
        .position(|game| game.app_id == app_id && game.exe == current_exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    let original = library[index].clone();
    let updating = original.use_steam_path;
    if library.iter().enumerate().any(|(other, game)| {
        other != index && game.app_id == app_id && game.exe.eq_ignore_ascii_case(&new_exe)
    }) {
        return Err("Another library entry already uses that executable".into());
    }

    if updating
        && (original.steam_app_id.as_deref() != Some(layout.steam_id.as_str())
            || original.install_dir.as_deref() != Some(layout.install_dir.as_str())
            || original.exe != new_exe)
    {
        return Err(
            "Remove the Steam integration before changing its App ID, folder, or executable"
                .to_string(),
        );
    }

    let source_bytes = fs::read(source).map_err(|error| error.to_string())?;
    let manifest = steam_manifest(&original.name, &layout.steam_id, &layout.install_dir);
    let mut old_manifest = None;
    let mut old_dummy = None;

    if updating {
        if !layout.install_root.is_dir() {
            return Err("The configured Steam install folder no longer exists".to_string());
        }
        ensure_existing_within(&layout.common, &layout.install_root)?;
        old_manifest = read_owned_artifact(
            &layout.steam_apps,
            &layout.manifest_path,
            original.steam_manifest_sha256.as_deref(),
        )?;
        old_dummy = read_owned_artifact(
            &layout.install_root,
            &layout.destination,
            original.steam_dummy_sha256.as_deref(),
        )?;
    } else {
        fs::create_dir_all(&layout.common).map_err(|error| error.to_string())?;
        if layout.manifest_path.exists() {
            return Err(format!(
                "A Steam manifest already exists at {}. Choose another App ID or remove it manually.",
                layout.manifest_path.display()
            ));
        }
        if layout.install_root.exists() {
            return Err(format!(
                "The Steam install folder already exists at {}. Choose a new, dedicated folder.",
                layout.install_root.display()
            ));
        }
        fs::create_dir(&layout.install_root).map_err(|error| error.to_string())?;
    }

    let manifest_bytes = manifest.into_bytes();
    let mut wrote_manifest = false;
    let mut wrote_dummy = false;
    let write_result = (|| -> Result<(), String> {
        let target_dir = layout
            .destination
            .parent()
            .ok_or_else(|| "Invalid executable destination".to_string())?;
        fs::create_dir_all(target_dir).map_err(|error| error.to_string())?;
        ensure_existing_within(steam_path, target_dir)?;
        write_artifact(&layout.manifest_path, &manifest_bytes, updating)?;
        wrote_manifest = true;
        write_artifact(&layout.destination, &source_bytes, updating)?;
        wrote_dummy = true;

        let game = &mut library[index];
        game.use_steam_path = true;
        game.steam_app_id = Some(layout.steam_id.clone());
        game.install_dir = Some(layout.install_dir.clone());
        game.exe = new_exe.clone();
        game.steam_exe_path = Some(layout.destination.to_string_lossy().to_string());
        game.steam_library_path = Some(steam_path.to_string_lossy().to_string());
        game.steam_previous_exe = if updating {
            original.steam_previous_exe.clone()
        } else {
            Some(original.exe.clone())
        };
        game.steam_manifest_sha256 = Some(sha256_hex(&manifest_bytes));
        game.steam_dummy_sha256 = Some(sha256_hex(&source_bytes));
        game.steam_created_install_dir = if updating {
            original.steam_created_install_dir
        } else {
            true
        };
        write_library(&state.paths.my_games_path, &library)
    })();

    if let Err(error) = write_result {
        if wrote_manifest {
            restore_artifact(
                &layout.manifest_path,
                old_manifest.as_deref(),
                Some(&manifest_bytes),
            );
        }
        if wrote_dummy {
            restore_artifact(
                &layout.destination,
                old_dummy.as_deref(),
                Some(&source_bytes),
            );
        }
        if !updating {
            let _ = remove_empty_steam_directories(&layout);
        }
        return Err(error);
    }

    Ok(SteamSetupResult {
        manifest_path: layout.manifest_path.to_string_lossy().to_string(),
        dest_exe_path: layout.destination.to_string_lossy().to_string(),
        steam_path: steam_path.to_string_lossy().to_string(),
        updated_game: library[index].clone(),
    })
}

#[tauri::command(async)]
fn setup_steam_integration(
    app: AppHandle,
    state: State<AppState>,
    request: SteamSetupRequest,
) -> Result<SteamSetupResult, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let steam_path = checked_steam_library(request.library_path.as_deref())?;
    let source = find_dummy_source(&app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    setup_steam_at(state.inner(), &source, &steam_path, &request)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamRemoveResult {
    removed_manifest: bool,
    removed_folder: bool,
    updated_game: Option<LibraryGame>,
}

fn remove_steam_at(
    state: &AppState,
    steam_path: &Path,
    app_id: &str,
    exe: &str,
) -> Result<SteamRemoveResult, String> {
    let running_key = format!("{app_id}::{exe}");
    if state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .contains_key(&running_key)
    {
        return Err("Stop the game before removing its Steam integration".to_string());
    }

    let mut library = load_library(&state.paths.my_games_path)?;
    let index = library
        .iter()
        .position(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    let original = library[index].clone();
    if !original.use_steam_path {
        return Err("Steam integration is not configured for this game".to_string());
    }

    let layout = steam_layout(
        steam_path,
        original.steam_app_id.as_deref().unwrap_or_default(),
        original.install_dir.as_deref().unwrap_or_default(),
        &original.exe,
    )?;
    reject_redirects(&layout.destination)?;
    reject_redirects(&layout.manifest_path)?;
    if layout.install_root.exists() {
        ensure_existing_within(&layout.common, &layout.install_root)?;
    }

    let old_manifest = read_owned_artifact(
        &layout.steam_apps,
        &layout.manifest_path,
        original.steam_manifest_sha256.as_deref(),
    )?;
    let old_dummy = read_owned_artifact(
        &layout.install_root,
        &layout.destination,
        original.steam_dummy_sha256.as_deref(),
    )?;

    let mut removed = Vec::new();
    let remove_result = (|| -> Result<(), String> {
        for (path, previous) in [
            (&layout.manifest_path, old_manifest.as_ref()),
            (&layout.destination, old_dummy.as_ref()),
        ] {
            if let Some(bytes) = previous {
                fs::remove_file(path).map_err(|error| error.to_string())?;
                removed.push((path.clone(), bytes.clone()));
            }
        }
        Ok(())
    })();
    if let Err(error) = remove_result {
        for (path, bytes) in &removed {
            restore_artifact(path, Some(bytes), None);
        }
        return Err(error);
    }

    let removed_folder = original.steam_created_install_dir
        && layout.install_root.exists()
        && remove_empty_steam_directories(&layout);
    library[index].use_steam_path = false;
    library[index].steam_exe_path = None;
    library[index].steam_library_path = None;
    library[index].steam_manifest_sha256 = None;
    library[index].steam_dummy_sha256 = None;
    library[index].steam_created_install_dir = false;
    if let Some(previous_exe) = original.steam_previous_exe {
        library[index].exe = previous_exe;
    }
    library[index].steam_previous_exe = None;

    if library.iter().enumerate().any(|(other, game)| {
        other != index
            && game.app_id == app_id
            && game.exe.eq_ignore_ascii_case(&library[index].exe)
    }) {
        for (path, bytes) in &removed {
            restore_artifact(path, Some(bytes), None);
        }
        return Err(
            "Restoring the original executable would duplicate another library entry".into(),
        );
    }

    if let Err(error) = write_library(&state.paths.my_games_path, &library) {
        for (path, bytes) in &removed {
            restore_artifact(path, Some(bytes), None);
        }
        return Err(error);
    }

    Ok(SteamRemoveResult {
        removed_manifest: old_manifest.is_some(),
        removed_folder,
        updated_game: Some(library[index].clone()),
    })
}

#[tauri::command(async)]
fn remove_steam_integration(
    state: State<AppState>,
    app_id: String,
    exe: String,
) -> Result<SteamRemoveResult, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let library = load_library(&state.paths.my_games_path)?;
    let game = library
        .iter()
        .find(|game| game.app_id == app_id && game.exe == exe)
        .ok_or_else(|| "Game not found in library".to_string())?;
    let steam_path = checked_steam_library(game.steam_library_path.as_deref())?;
    remove_steam_at(state.inner(), &steam_path, &app_id, &exe)
}

fn resolve_dummy_target(
    app: &AppHandle,
    paths: &AppPaths,
    game: &LibraryGame,
) -> Result<(PathBuf, PathBuf), String> {
    if game.use_steam_path {
        let steam_path = checked_steam_library(game.steam_library_path.as_deref())?;
        let layout = steam_layout(
            &steam_path,
            game.steam_app_id.as_deref().unwrap_or_default(),
            game.install_dir.as_deref().unwrap_or_default(),
            &game.exe,
        )?;
        ensure_existing_within(&layout.common, &layout.install_root)?;
        read_owned_artifact(
            &layout.steam_apps,
            &layout.manifest_path,
            game.steam_manifest_sha256.as_deref(),
        )?
        .ok_or_else(|| "The configured Steam manifest no longer exists".to_string())?;
        read_owned_artifact(
            &layout.install_root,
            &layout.destination,
            game.steam_dummy_sha256.as_deref(),
        )?
        .ok_or_else(|| "The configured Steam dummy no longer exists".to_string())?;
        let working_dir = layout
            .destination
            .parent()
            .ok_or_else(|| "Invalid executable destination".to_string())?
            .to_path_buf();
        return Ok((layout.destination, working_dir));
    }

    let source = find_dummy_source(app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    prepare_dummy(&source, paths, &game.app_id, &game.exe)
}

#[tauri::command(async)]
fn create_shortcut(
    app: AppHandle,
    state: State<AppState>,
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

#[tauri::command(async)]
fn launch_dummy(
    app: AppHandle,
    state: State<AppState>,
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

    update_tray_menu(&app, state.inner());

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

#[tauri::command(async)]
fn stop_dummy(app: AppHandle, state: State<AppState>, game_key: String) -> Result<bool, String> {
    let dummy = state
        .running
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&game_key);

    if let Some(dummy) = dummy {
        let name = dummy.name.clone();
        stop_running_dummy(dummy);
        update_tray_menu(&app, state.inner());
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

fn set_dummy_paused(state: &AppState, game_key: &str, paused: bool) -> Result<bool, String> {
    let running = state.running.lock().map_err(|error| error.to_string())?;
    let dummy = running
        .get(game_key)
        .ok_or_else(|| "The dummy is not running.".to_string())?;
    let Some(clock) = dummy.quest_clock.as_ref() else {
        return Ok(false);
    };
    let mut clock = clock.lock().map_err(|error| error.to_string())?;
    Ok(if paused {
        clock.pause()
    } else {
        clock.resume()
    })
}

#[tauri::command]
fn pause_dummy(state: State<AppState>, game_key: String) -> Result<bool, String> {
    set_dummy_paused(state.inner(), &game_key, true)
}

#[tauri::command]
fn resume_dummy(state: State<AppState>, game_key: String) -> Result<bool, String> {
    set_dummy_paused(state.inner(), &game_key, false)
}

#[tauri::command(async)]
fn stop_all_dummies(app: AppHandle, state: State<AppState>) -> Result<usize, String> {
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
    update_tray_menu(&app, state.inner());
    Ok(count)
}

#[tauri::command]
fn list_running_dummies(state: State<AppState>) -> Result<Vec<String>, String> {
    running_keys(&state)
}

#[tauri::command]
fn report_frontend_error(message: String) {
    log::error!(
        "UI operation: {}",
        message.chars().take(2000).collect::<String>()
    );
}

#[tauri::command(async)]
fn set_update_installing(state: State<AppState>, installing: bool) -> Result<(), String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    if installing && !running_keys(&state)?.is_empty() {
        return Err("Stop all active dummies before installing an update".into());
    }
    UPDATING.store(installing, Ordering::SeqCst);
    Ok(())
}

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
            get_steam_libraries,
            setup_steam_integration,
            remove_steam_integration,
            launch_dummy,
            stop_dummy,
            pause_dummy,
            resume_dummy,
            stop_all_dummies,
            list_running_dummies,
            set_update_installing,
            report_frontend_error
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

fn dispatch_shortcut(app: AppHandle, args: Vec<String>) {
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
        if let Err(error) = launch_dummy(app.clone(), app.state::<AppState>(), request) {
            log::error!("Shortcut launch failed: {error}");
            let _ = app.emit("dummy://error", error);
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
            ..Default::default()
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
            ..Default::default()
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
    fn migrates_hidden_dummy_window_to_visible_once() {
        let directory = temporary_directory("dummy-window-migration");
        fs::create_dir_all(&directory).expect("create temp dir");
        let paths = AppPaths {
            games_dir: directory.join("games"),
            settings_path: directory.join("settings.json"),
            my_games_path: directory.join("myGames.json"),
            gamelist_path: directory.join("gamelist.json"),
        };
        let settings = Settings {
            show_dummy_window: false,
            ..Default::default()
        };
        write_settings(&paths.settings_path, &settings).expect("write settings");

        migrate_dummy_window_visibility(&paths);
        assert!(load_settings(&paths.settings_path).show_dummy_window);

        let mut settings = load_settings(&paths.settings_path);
        settings.show_dummy_window = false;
        write_settings(&paths.settings_path, &settings).expect("write settings");
        migrate_dummy_window_visibility(&paths);
        assert!(!load_settings(&paths.settings_path).show_dummy_window);

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
