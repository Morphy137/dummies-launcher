//! Registered Steam-library discovery and fingerprint-verified artifact transactions.

use crate::library::{load_library, write_library};
use crate::models::{SteamRemoveResult, SteamSetupRequest, SteamSetupResult};
use crate::paths::{
    ensure_existing_within, reject_redirects, safe_path_component, sanitize_relative_exe,
};
use crate::resources::find_dummy_source;
use crate::state::{AppState, DATA_OPERATIONS};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::AppHandle;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

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

pub(crate) fn get_steam_path() -> Option<String> {
    find_steam_path().map(|path| path.to_string_lossy().to_string())
}

pub(crate) fn vdf_strings(raw: &str) -> Vec<String> {
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

pub(crate) fn get_steam_libraries() -> Vec<String> {
    steam_libraries()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

pub(crate) fn checked_steam_library(requested: Option<&str>) -> Result<PathBuf, String> {
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
pub(crate) struct SteamLayout {
    pub(crate) steam_id: String,
    pub(crate) install_dir: String,
    pub(crate) exe_relative: PathBuf,
    pub(crate) steam_apps: PathBuf,
    pub(crate) common: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) install_root: PathBuf,
    pub(crate) destination: PathBuf,
}

pub(crate) fn steam_layout(
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

pub(crate) fn steam_manifest(game_name: &str, steam_id: &str, install_dir: &str) -> String {
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

pub(crate) fn read_owned_artifact(
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

pub(crate) fn write_new_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

pub(crate) fn setup_steam_at(
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

pub(crate) fn setup_steam_integration(
    app: AppHandle,
    state: &AppState,
    request: SteamSetupRequest,
) -> Result<SteamSetupResult, String> {
    let _guard = DATA_OPERATIONS.lock().map_err(|error| error.to_string())?;
    let steam_path = checked_steam_library(request.library_path.as_deref())?;
    let source = find_dummy_source(&app).ok_or_else(|| {
        "Could not find dummy-game.exe. Run `pnpm build:dummy` first.".to_string()
    })?;
    setup_steam_at(state, &source, &steam_path, &request)
}

pub(crate) fn remove_steam_at(
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

pub(crate) fn remove_steam_integration(
    state: &AppState,
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
    remove_steam_at(state, &steam_path, &app_id, &exe)
}
