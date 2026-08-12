use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeProbeItem {
    pub id: String,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramSizeUpdate {
    pub id: String,
    pub estimated_size_kb: u64,
}

const MAX_FILES: u64 = 400_000;
const FORBIDDEN_ROOTS: &[&str] = &[
    r"c:\",
    r"c:\windows",
    r"c:\windows\system32",
    r"c:\program files",
    r"c:\program files (x86)",
    r"c:\programdata",
    r"c:\users",
];

/// Store / launcher roots that contain many apps — never attribute their whole
/// tree to a single Add/Remove entry (this caused Steam games to share ~244 GB).
const SHARED_LEAF_NAMES: &[&str] = &[
    "steam",
    "steam library",
    "steamapps",
    "epic games",
    "epicgameslauncher",
    "gog galaxy",
    "origin",
    "ea games",
    "ea desktop",
    "ubisoft",
    "ubisoft game launcher",
    "riot games",
    "battle.net",
    "blizzard",
    "xboxgames",
    "common files",
];

/// Walk install folders in the background and emit `program-size` as sizes are found.
/// `still_current` should return false when a newer probe superseded this one.
pub fn probe_missing_sizes(
    app: AppHandle,
    items: Vec<SizeProbeItem>,
    still_current: impl Fn() -> bool,
) {
    let mut claimed_roots = HashSet::new();

    for item in items {
        if !still_current() {
            return;
        }

        let Some(root) = resolve_scan_root(
            item.install_location.as_deref(),
            item.uninstall_string.as_deref(),
        ) else {
            continue;
        };

        let root_key = normalize_path_key(&root);
        if !claimed_roots.insert(root_key) {
            // Another program already owns this folder — don't duplicate its size.
            continue;
        }

        let bytes = dir_size_bytes(&root);
        if !still_current() {
            return;
        }
        if bytes == 0 {
            continue;
        }

        let kb = (bytes + 1023) / 1024;
        let _ = app.emit(
            "program-size",
            ProgramSizeUpdate {
                id: item.id,
                estimated_size_kb: kb,
            },
        );
    }

    if still_current() {
        let _ = app.emit("program-size-finished", ());
    }
}

fn resolve_scan_root(
    install_location: Option<&str>,
    uninstall_string: Option<&str>,
) -> Option<PathBuf> {
    if let Some(loc) = install_location.map(str::trim).filter(|s| !s.is_empty()) {
        let path = PathBuf::from(trim_quotes(loc));
        if is_safe_scan_root(&path) {
            return Some(path);
        }
    }

    // Only fall back to the uninstaller directory when the binary itself looks like
    // an uninstaller. Never use steam.exe / launcher parents (shared store roots).
    if let Some(cmd) = uninstall_string.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(exe) = extract_exe_path(cmd) {
            if exe_looks_like_uninstaller(&exe) {
                if let Some(parent) = Path::new(&exe).parent() {
                    if is_safe_scan_root(parent) {
                        return Some(parent.to_path_buf());
                    }
                }
            }
        }
    }

    None
}

fn trim_quotes(s: &str) -> &str {
    s.trim().trim_matches('"').trim()
}

fn extract_exe_path(command_line: &str) -> Option<String> {
    let input = command_line.trim();
    if input.is_empty() {
        return None;
    }

    if input.starts_with('"') {
        let rest = &input[1..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }

    let lower = input.to_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(idx) = lower.find(ext) {
            return Some(input[..idx + ext.len()].to_string());
        }
    }

    input.split_whitespace().next().map(str::to_string)
}

fn exe_looks_like_uninstaller(exe: &str) -> bool {
    let name = Path::new(exe)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains("uninstall") || name == "uninst.exe" {
        return true;
    }
    // Inno Setup: unins000.exe
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    match stem.strip_prefix("unins") {
        Some(rest) if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) => true,
        _ => false,
    }
}

fn normalize_path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .trim_end_matches(['\\', '/'])
        .to_ascii_lowercase()
}

fn is_safe_scan_root(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    let canon = canonical.to_string_lossy();
    let normalized = canon
        .strip_prefix(r"\\?\")
        .unwrap_or(canon.as_ref())
        .trim_end_matches(['\\', '/']);
    let lower = normalized.to_ascii_lowercase();

    if lower.contains(r"\temp\")
        || lower.contains(r"\tmp\")
        || lower.ends_with(r"\temp")
        || lower.ends_with(r"\tmp")
        || lower.contains(r"\appdata\local\temp")
    {
        return false;
    }

    for forbidden in FORBIDDEN_ROOTS {
        if lower == *forbidden {
            return false;
        }
    }

    if is_shared_store_path(&lower) {
        return false;
    }

    // Require enough depth so we never scan a drive or top-level program folder.
    let components = Path::new(normalized)
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count();
    components >= 2
}

fn is_shared_store_path(lower: &str) -> bool {
    // Per-game folders under Steam are OK: ...\steamapps\common\<Game>
    if let Some(idx) = lower.find(r"\steamapps\common\") {
        let after = &lower[idx + r"\steamapps\common\".len()..];
        return after.is_empty();
    }

    // Anything under \Steam\ that is NOT steamapps\common\<game> is a shared client tree.
    if let Some(idx) = lower.find(r"\steam\") {
        let after_steam = &lower[idx + r"\steam\".len()..];
        if !after_steam.starts_with(r"steamapps\common\") {
            return true;
        }
    }
    if lower.ends_with(r"\steam") || lower.ends_with(r"\steam library") {
        return true;
    }

    let leaf = lower.rsplit('\\').next().unwrap_or("");
    if SHARED_LEAF_NAMES.contains(&leaf) {
        return true;
    }

    if lower.ends_with(r"\epic games") || lower.ends_with(r"\gog galaxy\games") {
        return true;
    }

    false
}

fn dir_size_bytes(root: &Path) -> u64 {
    let mut total = 0u64;
    let mut files = 0u64;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };

            if meta.file_type().is_symlink() {
                continue;
            }

            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
                files += 1;
                if files >= MAX_FILES {
                    return total;
                }
            }
        }
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_quoted_and_unquoted_exe() {
        assert_eq!(
            extract_exe_path(r#""C:\Program Files\App\Uninstall.exe" /S"#).as_deref(),
            Some(r"C:\Program Files\App\Uninstall.exe")
        );
        assert_eq!(
            extract_exe_path(r"C:\Program Files\App\Uninstall.exe /S").as_deref(),
            Some(r"C:\Program Files\App\Uninstall.exe")
        );
    }

    #[test]
    fn steam_client_root_is_shared_but_game_folder_is_not() {
        assert!(is_shared_store_path(
            r"c:\program files (x86)\steam"
        ));
        assert!(is_shared_store_path(
            r"c:\program files (x86)\steam\bin"
        ));
        assert!(is_shared_store_path(
            r"c:\program files (x86)\steam\steamapps"
        ));
        assert!(!is_shared_store_path(
            r"c:\program files (x86)\steam\steamapps\common\skyrim"
        ));
        assert!(!is_shared_store_path(
            r"d:\steamlibrary\steamapps\common\dayz"
        ));
    }

    #[test]
    fn steam_exe_is_not_treated_as_uninstaller() {
        assert!(!exe_looks_like_uninstaller(
            r"C:\Program Files (x86)\Steam\steam.exe"
        ));
        assert!(exe_looks_like_uninstaller(
            r"C:\Program Files\App\Uninstall.exe"
        ));
        assert!(exe_looks_like_uninstaller(
            r"C:\Program Files\Unity Hub\Uninstall Unity Hub.exe"
        ));
    }

    #[test]
    fn uninstall_fallback_ignores_steam_launcher() {
        assert!(resolve_scan_root(
            None,
            Some(r#""C:\Program Files (x86)\Steam\steam.exe" steam://uninstall/440"#),
        )
        .is_none());
    }
}
