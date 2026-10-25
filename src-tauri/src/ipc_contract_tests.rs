use crate::models::{
    AddGameRequest, Bootstrap, DatabaseGame, ExitedPayload, LaunchRequest, LaunchResult,
    LibraryGame, QuestTick, Settings, SteamRemoveResult, SteamSetupRequest, SteamSetupResult,
};
use serde_json::{Value, json};

#[test]
fn requests_keep_camel_case_identity_and_steam_destination_fields() {
    let launch: LaunchRequest = serde_json::from_value(json!({
        "appId": "123", "exe": "original.exe"
    }))
    .unwrap();
    assert_eq!(launch.app_id, "123");
    assert_eq!(launch.exe, "original.exe");

    let steam: SteamSetupRequest = serde_json::from_value(json!({
        "appId": "123", "currentExe": "original.exe", "steamAppId": "456",
        "installDir": "DedicatedDummy", "exe": "bin/game.exe", "libraryPath": null
    }))
    .unwrap();
    assert_eq!(steam.app_id, launch.app_id);
    assert_eq!(steam.current_exe, launch.exe);
    assert_eq!(steam.steam_app_id, "456");
    assert_eq!(steam.install_dir, "DedicatedDummy");
    assert_eq!(steam.exe, "bin/game.exe");
    assert!(steam.library_path.is_none());

    let add: AddGameRequest = serde_json::from_value(json!({
        "id": "123", "name": "Example", "exe": "original.exe", "usesNewDetection": true,
        "steamAppId": "456", "iconHash": null, "coverImageHash": null
    }))
    .unwrap();
    assert_eq!(add.id, launch.app_id);
    assert!(add.uses_new_detection);
    assert_eq!(add.steam_app_id.as_deref(), Some("456"));
}

#[test]
fn persisted_models_preserve_unknown_fields_without_runtime_state() {
    let settings: Settings = serde_json::from_value(json!({"futureSetting": true})).unwrap();
    let settings = serde_json::to_value(settings).unwrap();
    assert_eq!(settings["schemaVersion"], 1);
    assert_eq!(settings["futureSetting"], true);
    assert_eq!(settings["questDurationMinutes"], 15);

    let game: LibraryGame = serde_json::from_value(json!({
        "appId": "123", "name": "Example", "exe": "original.exe", "futureMetadata": "keep"
    }))
    .unwrap();
    let game = serde_json::to_value(game).unwrap();
    assert_eq!(game["futureMetadata"], "keep");
    assert_eq!(game["isFavorite"], false);
    assert_eq!(game["steamExePath"], Value::Null);
    assert_eq!(game["steamLibraryPath"], Value::Null);
    assert!(game.get("running").is_none());
}

#[test]
fn runtime_payloads_keep_nullable_exit_codes_and_timer_snapshots() {
    let exited = serde_json::to_value(ExitedPayload {
        key: "123::original.exe".into(),
        name: "Example".into(),
        code: None,
    })
    .unwrap();
    assert_eq!(
        exited,
        json!({"key": "123::original.exe", "name": "Example", "code": null})
    );

    let tick = QuestTick {
        key: "123::original.exe".into(),
        name: "Example".into(),
        timer_id: 7,
        remaining_seconds: 60,
        phase: "syncing".into(),
        percent: 95,
        paused: false,
    };
    let bootstrap = serde_json::to_value(Bootstrap {
        settings: Settings::default(),
        system_locale: "es".into(),
        running_keys: vec![tick.key.clone()],
        library: vec![],
        version: "test".into(),
        portable: false,
        quest_ticks: vec![tick],
    })
    .unwrap();
    assert_eq!(bootstrap["runningKeys"], json!(["123::original.exe"]));
    assert_eq!(
        bootstrap["questTicks"],
        json!([{
            "key": "123::original.exe", "name": "Example", "timerId": 7,
            "remainingSeconds": 60, "phase": "syncing", "percent": 95, "paused": false
        }])
    );
    assert_eq!(bootstrap["systemLocale"], "es");
    assert_eq!(bootstrap["portable"], false);
}

#[test]
fn command_results_keep_frontend_field_names_and_hide_catalog_search_index() {
    assert_eq!(
        serde_json::to_value(LaunchResult {
            game_key: "123::original.exe".into(),
            exe_path: "original.exe".into(),
            already_running: false,
        })
        .unwrap(),
        json!({"gameKey": "123::original.exe", "exePath": "original.exe", "alreadyRunning": false})
    );

    let game = LibraryGame::default();
    let setup = serde_json::to_value(SteamSetupResult {
        manifest_path: "manifest.acf".into(),
        dest_exe_path: "game.exe".into(),
        steam_path: "Steam".into(),
        updated_game: game.clone(),
    })
    .unwrap();
    assert_eq!(setup["updatedGame"], serde_json::to_value(game).unwrap());
    assert_eq!(setup["manifestPath"], "manifest.acf");
    assert_eq!(setup["destExePath"], "game.exe");
    assert_eq!(setup["steamPath"], "Steam");
    assert_eq!(
        serde_json::to_value(SteamRemoveResult {
            removed_manifest: true,
            removed_folder: false,
            updated_game: None,
        })
        .unwrap(),
        json!({"removedManifest": true, "removedFolder": false, "updatedGame": null})
    );

    let catalog = serde_json::to_value(DatabaseGame {
        id: "123".into(),
        name: "Example".into(),
        exe: "game.exe".into(),
        name_lower: "example".into(),
        is_launcher: false,
        uses_new_detection: true,
        steam_app_id: None,
        icon_hash: None,
        cover_image_hash: None,
    })
    .unwrap();
    assert_eq!(catalog["usesNewDetection"], true);
    assert!(catalog.get("nameLower").is_none());
    assert!(catalog.get("name_lower").is_none());
}
