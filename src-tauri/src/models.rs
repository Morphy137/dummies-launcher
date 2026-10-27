//! Persisted models and serialized IPC/event payloads, independent of the desktop runtime.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Settings {
    pub(crate) schema_version: u32,
    pub(crate) theme: String,
    pub(crate) language: String,
    pub(crate) show_dummy_window: bool,
    pub(crate) minimize_to_tray: bool,
    pub(crate) cards_per_row: u8,
    pub(crate) max_card_width: u16,
    pub(crate) quest_timer_enabled: bool,
    pub(crate) quest_duration_minutes: u32,
    pub(crate) quest_sync_buffer_seconds: u32,
    pub(crate) auto_stop_on_complete: bool,
    pub(crate) notify_on_complete: bool,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
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
            max_card_width: 180,
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
pub(crate) struct Bootstrap {
    pub(crate) settings: Settings,
    pub(crate) system_locale: String,
    pub(crate) running_keys: Vec<String>,
    pub(crate) library: Vec<LibraryGame>,
    pub(crate) version: String,
    pub(crate) portable: bool,
    pub(crate) quest_ticks: Vec<QuestTick>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LibraryGame {
    pub(crate) app_id: String,
    pub(crate) name: String,
    pub(crate) exe: String,
    pub(crate) is_favorite: bool,
    pub(crate) steam_app_id: Option<String>,
    pub(crate) icon_hash: Option<String>,
    pub(crate) cover_image_hash: Option<String>,
    pub(crate) requires_steam: bool,
    pub(crate) use_steam_path: bool,
    pub(crate) install_dir: Option<String>,
    pub(crate) steam_exe_path: Option<String>,
    pub(crate) steam_library_path: Option<String>,
    pub(crate) steam_previous_exe: Option<String>,
    pub(crate) steam_manifest_sha256: Option<String>,
    pub(crate) steam_dummy_sha256: Option<String>,
    pub(crate) steam_created_install_dir: bool,
    #[serde(flatten)]
    pub(crate) extra: serde_json::Map<String, serde_json::Value>,
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
pub(crate) struct DatabaseGame {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) exe: String,
    pub(crate) is_launcher: bool,
    pub(crate) uses_new_detection: bool,
    pub(crate) steam_app_id: Option<String>,
    pub(crate) icon_hash: Option<String>,
    pub(crate) cover_image_hash: Option<String>,
    #[serde(skip)]
    pub(crate) name_lower: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabasePage {
    pub(crate) items: Vec<DatabaseGame>,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
    pub(crate) has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncResult {
    pub(crate) updated: bool,
    pub(crate) count: usize,
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamSetupResult {
    pub(crate) manifest_path: String,
    pub(crate) dest_exe_path: String,
    pub(crate) steam_path: String,
    pub(crate) updated_game: LibraryGame,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamSetupRequest {
    pub(crate) app_id: String,
    pub(crate) current_exe: String,
    pub(crate) steam_app_id: String,
    pub(crate) install_dir: String,
    pub(crate) exe: String,
    pub(crate) library_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddGameRequest {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) exe: String,
    pub(crate) uses_new_detection: bool,
    pub(crate) steam_app_id: Option<String>,
    pub(crate) icon_hash: Option<String>,
    pub(crate) cover_image_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchRequest {
    pub(crate) app_id: String,
    pub(crate) exe: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchResult {
    pub(crate) game_key: String,
    pub(crate) exe_path: String,
    pub(crate) already_running: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExitedPayload {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) code: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestTick {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) timer_id: u64,
    pub(crate) remaining_seconds: u64,
    pub(crate) phase: String,
    pub(crate) percent: u8,
    pub(crate) paused: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamRemoveResult {
    pub(crate) removed_manifest: bool,
    pub(crate) removed_folder: bool,
    pub(crate) updated_game: Option<LibraryGame>,
}
