use crate::catalog::AppExecutable;
use crate::catalog::DetectableApp;
use crate::catalog::ThirdPartySku;
use crate::catalog::page_database;
use crate::catalog::to_database_games;
use crate::dummy::prepare_dummy;
use crate::migration::migrate_dummy_window_visibility;
use crate::models::Settings;
use crate::paths::sanitize_component;
use crate::paths::sanitize_relative_exe;
use crate::settings::load_settings;
use crate::settings::normalize_settings;
use crate::settings::write_settings;
use crate::state::AppPaths;
use std::fs;
use std::path::PathBuf;

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
        max_card_width: 999,
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
    assert_eq!(settings.max_card_width, 180);
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
fn supports_dense_bounded_cards_and_defaults_older_settings() {
    let old: Settings = serde_json::from_str(r#"{"cardsPerRow":8}"#).unwrap();
    assert_eq!(normalize_settings(old).max_card_width, 180);
    let settings = normalize_settings(Settings {
        cards_per_row: 24,
        max_card_width: 240,
        ..Default::default()
    });
    let json = serde_json::to_value(&settings).unwrap();
    assert_eq!(json["cardsPerRow"], 24);
    assert_eq!(json["maxCardWidth"], 240);
    let restored: Settings = serde_json::from_value(json).unwrap();
    assert_eq!(restored.max_card_width, 240);
    assert_eq!(
        normalize_settings(Settings {
            max_card_width: 0,
            ..Default::default()
        })
        .max_card_width,
        180
    );
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
