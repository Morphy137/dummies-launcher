//! Windows-safe path components, reparse-point rejection and canonical containment.

use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn sanitize_component(value: &str) -> Option<String> {
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

pub(crate) fn sanitize_relative_exe(value: &str) -> Option<PathBuf> {
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

pub(crate) fn safe_path_component(value: &str) -> bool {
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

pub(crate) fn reject_redirects(path: &Path) -> Result<(), String> {
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

pub(crate) fn ensure_existing_within(base: &Path, path: &Path) -> Result<(), String> {
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
