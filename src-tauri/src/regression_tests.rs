use crate::catalog::{load_database, validate_catalog};
use crate::dummy::prepare_dummy;
use crate::library::{
    add_game, load_library, remove_game, toggle_favorite, update_game_executable, write_library,
};
use crate::migration::migrate_legacy_from;
use crate::models::{AddGameRequest, LibraryGame, Settings, SteamSetupRequest, SteamSetupResult};
use crate::paths::{ensure_existing_within, sanitize_relative_exe};
use crate::processes::{RunningDummy, stop_all_internal};
use crate::settings::load_settings;
use crate::state::{AppPaths, AppState};
use crate::steam::{
    read_owned_artifact, remove_steam_at, setup_steam_at, steam_layout, steam_manifest,
    vdf_strings, write_new_artifact,
};
use crate::timers::QuestClock;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    steam: PathBuf,
    state: AppState,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "dummies-regression-{}-{}",
            std::process::id(),
            NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("games")).unwrap();
        let source = root.join("source.exe");
        fs::write(&source, b"dummy-v1").unwrap();
        let steam = root.join("Steam");
        fs::create_dir_all(&steam).unwrap();
        let state = AppState {
            paths: AppPaths {
                games_dir: root.join("games"),
                settings_path: root.join("settings.json"),
                my_games_path: root.join("myGames.json"),
                gamelist_path: root.join("gamelist.json"),
            },
            settings: Mutex::new(Settings::default()),
            database: RwLock::new(Vec::new()),
            running: Arc::new(Mutex::new(HashMap::new())),
        };
        write_library(
            &state.paths.my_games_path,
            &[LibraryGame {
                app_id: "123".into(),
                name: "Example".into(),
                exe: "original.exe".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        Self {
            root,
            source,
            steam,
            state,
        }
    }

    fn install(&self) -> SteamSetupResult {
        self.setup("456", "DedicatedDummy", "bin/game.exe").unwrap()
    }

    fn setup(&self, steam_id: &str, folder: &str, exe: &str) -> Result<SteamSetupResult, String> {
        setup_steam_at(
            &self.state,
            &self.source,
            &self.steam,
            &SteamSetupRequest {
                app_id: "123".into(),
                current_exe: "original.exe".into(),
                steam_app_id: steam_id.into(),
                install_dir: folder.into(),
                exe: exe.into(),
                library_path: None,
            },
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        stop_all_internal(&self.state);
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn steam_removal_preserves_unrelated_files_and_restores_identity() {
    let f = Fixture::new();
    let installed = f.install();
    let unrelated = f.steam.join("steamapps/common/DedicatedDummy/save.dat");
    fs::write(&unrelated, b"keep").unwrap();
    let result = remove_steam_at(&f.state, &f.steam, "123", &installed.updated_game.exe).unwrap();
    assert!(!result.removed_folder);
    assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
    assert!(!Path::new(&installed.dest_exe_path).exists());
    assert_eq!(result.updated_game.unwrap().exe, "original.exe");
}

#[test]
fn steam_refuses_existing_installation_and_manifest() {
    let f = Fixture::new();
    let root = f.steam.join("steamapps/common/DedicatedDummy");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("real.exe"), b"real game").unwrap();
    assert!(f.setup("456", "DedicatedDummy", "real.exe").is_err());
    assert_eq!(fs::read(root.join("real.exe")).unwrap(), b"real game");
    let manifest = f.steam.join("steamapps/appmanifest_789.acf");
    fs::write(&manifest, b"real manifest").unwrap();
    assert!(f.setup("789", "Unused", "game.exe").is_err());
    assert_eq!(fs::read(manifest).unwrap(), b"real manifest");
    assert!(!root.parent().unwrap().join("Unused").exists());
}

#[test]
fn steam_refuses_modified_files_before_deleting_anything() {
    let f = Fixture::new();
    let installed = f.install();
    fs::write(&installed.dest_exe_path, b"replaced externally").unwrap();
    assert!(remove_steam_at(&f.state, &f.steam, "123", &installed.updated_game.exe).is_err());
    assert!(Path::new(&installed.manifest_path).exists());
    assert_eq!(
        fs::read(installed.dest_exe_path).unwrap(),
        b"replaced externally"
    );
}

#[test]
fn failed_install_rolls_back_only_its_own_files() {
    let f = Fixture::new();
    // A directory at the library temporary/backup location prevents persistence.
    fs::create_dir(f.state.paths.my_games_path.with_extension("json.bak")).unwrap();
    assert!(f.setup("456", "DedicatedDummy", "bin/game.exe").is_err());
    assert!(!f.steam.join("steamapps/appmanifest_456.acf").exists());
    assert!(!f.steam.join("steamapps/common/DedicatedDummy").exists());
    assert_eq!(
        load_library(&f.state.paths.my_games_path).unwrap()[0].exe,
        "original.exe"
    );
}

#[test]
fn failed_new_file_creation_never_deletes_existing_data() {
    let f = Fixture::new();
    let path = f.root.join("existing");
    fs::write(&path, b"keep").unwrap();
    assert!(write_new_artifact(&path, b"replace").is_err());
    assert_eq!(fs::read(path).unwrap(), b"keep");
}

#[test]
fn rejects_absolute_reserved_and_redirected_paths() {
    let f = Fixture::new();
    for exe in [
        "../outside.exe",
        "C:\\outside.exe",
        "/outside.exe",
        "\\\\host\\share\\a.exe",
        "CON.exe",
        "bin/./a.exe",
        "bin/../a.exe",
        "game.exe:stream",
        "game. /a.exe",
    ] {
        assert!(sanitize_relative_exe(exe).is_none(), "{exe}");
    }
    for folder in ["..", "../other", "C:\\other", "CON", "folder."] {
        assert!(steam_layout(&f.steam, "456", folder, "game.exe").is_err());
    }
    assert!(steam_layout(&f.steam, "../456", "Dummy", "game.exe").is_err());
    let common = f.steam.join("steamapps/common");
    fs::create_dir_all(&common).unwrap();
    // Directory junctions work without Windows developer-mode symlink privileges.
    let link = common.join("Redirected");
    use std::os::windows::process::CommandExt;
    let status = Command::new("cmd")
        .raw_arg(format!(
            "/C mklink /J \"{}\" \"{}\"",
            link.display().to_string().replace('/', "\\"),
            f.root.display()
        ))
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(ensure_existing_within(&common, &link).is_err());
    fs::remove_dir(link).unwrap();
}

#[test]
fn corrupt_library_is_reported_and_never_overwritten() {
    let f = Fixture::new();
    fs::write(&f.state.paths.my_games_path, b"{corrupt").unwrap();
    assert!(load_library(&f.state.paths.my_games_path).is_err());
    assert!(write_library(&f.state.paths.my_games_path, &[]).is_err());
    assert_eq!(fs::read(&f.state.paths.my_games_path).unwrap(), b"{corrupt");
}

#[test]
fn backup_keeps_previous_valid_library() {
    let f = Fixture::new();
    write_library(&f.state.paths.my_games_path, &[]).unwrap();
    let backup = f.state.paths.my_games_path.with_extension("json.bak");
    assert_eq!(load_library(&backup).unwrap()[0].exe, "original.exe");
}

#[test]
fn same_size_dummy_update_is_not_skipped() {
    let f = Fixture::new();
    let (destination, _) = prepare_dummy(&f.source, &f.state.paths, "123", "game.exe").unwrap();
    fs::write(&f.source, b"dummy-v2").unwrap();
    prepare_dummy(&f.source, &f.state.paths, "123", "game.exe").unwrap();
    assert_eq!(fs::read(destination).unwrap(), b"dummy-v2");
}

#[test]
fn validates_remote_catalog_and_escapes_manifest_names() {
    for invalid in ["[]", "{}", "<html>error</html>", "[{\"id\":\"123\"}]"] {
        assert!(validate_catalog(invalid).is_err());
    }
    assert!(validate_catalog("[{\"id\":\"123\",\"name\":\"Game\"}]").is_ok());
    assert!(
        steam_manifest("Game \"quoted\"\nNext", "123", "Dummy")
            .contains("Game \\\"quoted\\\" Next")
    );
}

#[test]
fn migration_imports_missing_files_across_legacy_directories() {
    let f = Fixture::new();
    let legacy = f.root.join("legacy");
    let first = legacy.join("discord-fake-game-launcher-ui");
    let second = legacy.join("Discord Fake Game Launcher");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("settings.json"), b"{\"language\":\"es\"}").unwrap();
    fs::write(
        second.join("gamelist.json"),
        b"[{\"id\":\"123\",\"name\":\"Game\"}]",
    )
    .unwrap();
    migrate_legacy_from(&legacy, &f.state.paths);
    assert_eq!(load_settings(&f.state.paths.settings_path).language, "es");
    assert_eq!(load_database(&f.state.paths.gamelist_path).len(), 1);
    assert!(
        f.state
            .paths
            .settings_path
            .with_extension("json.bak")
            .exists()
    );
    assert!(first.join("settings.json").exists());
    fs::write(first.join("settings.json"), b"{\"language\":\"en\"}").unwrap();
    migrate_legacy_from(&legacy, &f.state.paths);
    assert_eq!(load_settings(&f.state.paths.settings_path).language, "es");
}

#[test]
fn refuses_duplicate_library_identities_and_unverified_legacy_files() {
    let f = Fixture::new();
    let game = load_library(&f.state.paths.my_games_path)
        .unwrap()
        .remove(0);
    let duplicate = LibraryGame {
        exe: "ORIGINAL.EXE".into(),
        ..game.clone()
    };
    assert!(write_library(&f.state.paths.my_games_path, &[game, duplicate]).is_err());
    let file = f.root.join("legacy.exe");
    fs::write(&file, b"dummy-v1").unwrap();
    assert!(read_owned_artifact(&f.root, &file, None).is_err());
    assert!(file.exists());
    let tokens = vdf_strings("\"libraryfolders\" { \"1\" { \"path\" \"D:\\\\SteamLibrary\" } }");
    assert!(
        tokens
            .windows(2)
            .any(|pair| pair == ["path", "D:\\SteamLibrary"])
    );
}

#[test]
fn dropped_dummy_cancels_timer_and_clock_pause_is_idempotent() {
    let stop = Arc::new(AtomicBool::new(false));
    let mut child = Command::new("cmd")
        .args(["/C", "exit", "0"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    child.wait().unwrap();
    drop(RunningDummy {
        child,
        stdin,
        name: "Test".into(),
        timer_stop: Some(stop.clone()),
        quest_clock: None,
        last_tick: None,
    });
    assert!(stop.load(Ordering::SeqCst));
    let mut clock = QuestClock::new();
    assert!(clock.pause());
    assert!(!clock.pause());
    assert!(clock.resume());
    assert!(!clock.resume());
}

#[test]
fn library_services_preserve_identity_and_reject_duplicate_executable_edits() {
    let f = Fixture::new();
    let added = add_game(
        &f.state,
        AddGameRequest {
            id: "123".into(),
            name: "Another entry".into(),
            exe: "other.exe".into(),
            uses_new_detection: false,
            steam_app_id: None,
            icon_hash: None,
            cover_image_hash: None,
        },
    )
    .unwrap();
    assert_eq!(added.exe, "other.exe");
    assert!(
        update_game_executable(
            &f.state,
            "123".into(),
            "original.exe".into(),
            "OTHER.EXE".into()
        )
        .is_err()
    );
    let favorite = toggle_favorite(&f.state, "123".into(), "other.exe".into()).unwrap();
    assert!(favorite.is_favorite);
    let library = load_library(&f.state.paths.my_games_path).unwrap();
    assert_eq!(library.len(), 2);
    assert!(!library[0].is_favorite);
    assert!(remove_game(&f.state, "123".into(), "other.exe".into()).unwrap());
    assert_eq!(
        load_library(&f.state.paths.my_games_path).unwrap()[0].exe,
        "original.exe"
    );
}

#[test]
fn extracted_library_mutations_keep_the_shared_operation_lock() {
    let f = Fixture::new();
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let state = &f.state;
            scope.spawn(move || {
                for _ in 0..9 {
                    toggle_favorite(state, "123".into(), "original.exe".into()).unwrap();
                }
            });
        }
    });
    let library = load_library(&f.state.paths.my_games_path).unwrap();
    assert_eq!(library.len(), 1);
    assert!(!library[0].is_favorite);
}

#[test]
fn library_services_refuse_mutation_while_a_dummy_is_tracked() {
    let f = Fixture::new();
    let mut child = Command::new("cmd")
        .args(["/C", "set /p fixture="])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    f.state.running.lock().unwrap().insert(
        "123::original.exe".into(),
        RunningDummy {
            child,
            stdin,
            name: "Example".into(),
            timer_stop: None,
            quest_clock: None,
            last_tick: None,
        },
    );
    assert!(remove_game(&f.state, "123".into(), "original.exe".into()).is_err());
    assert!(
        update_game_executable(
            &f.state,
            "123".into(),
            "original.exe".into(),
            "other.exe".into()
        )
        .is_err()
    );
    assert!(f.setup("456", "DedicatedDummy", "game.exe").is_err());
    assert_eq!(
        load_library(&f.state.paths.my_games_path).unwrap()[0].exe,
        "original.exe"
    );
}
