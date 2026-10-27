//! Remote catalog validation, caching, executable selection and bounded searches.

use crate::models::{DatabaseGame, DatabasePage, SyncResult};
use crate::paths::sanitize_component;
use crate::state::AppState;
use crate::storage;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

static CATALOG_SYNC: Mutex<()> = Mutex::new(());
const DISCORD_DETECTABLE_URL: &str = "https://discord.com/api/applications/detectable";

#[derive(Debug, Deserialize)]
pub(crate) struct DetectableApp {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) executables: Option<Vec<AppExecutable>>,
    pub(crate) icon_hash: Option<String>,
    pub(crate) cover_image_hash: Option<String>,
    pub(crate) third_party_skus: Option<Vec<ThirdPartySku>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AppExecutable {
    pub(crate) name: Option<String>,
    pub(crate) os: Option<String>,
    pub(crate) is_launcher: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ThirdPartySku {
    pub(crate) distributor: Option<String>,
    pub(crate) id: Option<String>,
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

pub(crate) fn to_database_games(apps: Vec<DetectableApp>) -> Vec<DatabaseGame> {
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

pub(crate) fn load_database(path: &Path) -> Vec<DatabaseGame> {
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

pub(crate) fn validate_catalog(raw: &str) -> Result<(), String> {
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

pub(crate) fn page_database(
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

pub(crate) fn sync_gamelist(state: &AppState) -> Result<SyncResult, String> {
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

pub(crate) fn get_database_page(
    state: &AppState,
    filter: String,
    offset: usize,
    limit: usize,
) -> Result<DatabasePage, String> {
    let database = state.database.read().map_err(|error| error.to_string())?;
    Ok(page_database(&database, &filter, offset, limit))
}
