use serde::Serialize;
use winreg::enums::*;
use winreg::RegKey;

const APP_DISPLAY_NAMES: &[&str] = &[
    "Uninstall Many Programs",
    "Uninstall a Program (Bulk Edition)",
    "Uninstall Manager",
    "Ultimate Uninstaller",
    "Bulk Uninstaller",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledProgram {
    pub id: String,
    pub display_name: String,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub install_date: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub quiet_uninstall_string: Option<String>,
    pub estimated_size_kb: Option<u64>,
    pub windows_installer: bool,
    pub system_component: bool,
    pub protected: bool,
    pub hive: String,
    pub category: Option<String>,
}

struct HiveSource {
    hive_name: &'static str,
    root: RegKey,
    path: &'static str,
}

pub fn list_installed_programs(show_system: bool) -> Result<Vec<InstalledProgram>, String> {
    let sources = [
        HiveSource {
            hive_name: "HKLM",
            root: RegKey::predef(HKEY_LOCAL_MACHINE),
            path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        },
        HiveSource {
            hive_name: "HKLM-WOW64",
            root: RegKey::predef(HKEY_LOCAL_MACHINE),
            path: r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        },
        HiveSource {
            hive_name: "HKCU",
            root: RegKey::predef(HKEY_CURRENT_USER),
            path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        },
        HiveSource {
            hive_name: "HKCU-WOW64",
            root: RegKey::predef(HKEY_CURRENT_USER),
            path: r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        },
    ];

    let mut programs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for source in &sources {
        let Ok(key) = source.root.open_subkey(source.path) else {
            continue;
        };

        for subkey_name in key.enum_keys().filter_map(Result::ok) {
            let Ok(subkey) = key.open_subkey(&subkey_name) else {
                continue;
            };

            let Some(program) = read_program(source, &subkey_name, &subkey) else {
                continue;
            };

            if program.system_component && !show_system {
                continue;
            }

            let dedupe_key = format!(
                "{}|{}|{}",
                program.display_name.to_lowercase(),
                program.publisher.clone().unwrap_or_default().to_lowercase(),
                program.display_version.clone().unwrap_or_default()
            );
            if !seen.insert(dedupe_key) {
                continue;
            }

            programs.push(program);
        }
    }

    crate::categories::enrich_categories(&mut programs);

    programs.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });

    Ok(programs)
}

fn read_program(source: &HiveSource, subkey_name: &str, subkey: &RegKey) -> Option<InstalledProgram> {
    let display_name: String = subkey.get_value("DisplayName").ok()?;
    let display_name = display_name.trim().to_string();
    if display_name.is_empty() {
        return None;
    }

    let publisher: Option<String> = subkey.get_value("Publisher").ok();
    let display_version: Option<String> = subkey.get_value("DisplayVersion").ok();
    let install_date: Option<String> = subkey.get_value("InstallDate").ok();
    let install_location: Option<String> = subkey.get_value("InstallLocation").ok();
    let uninstall_string: Option<String> = subkey.get_value("UninstallString").ok();
    let quiet_uninstall_string: Option<String> = subkey.get_value("QuietUninstallString").ok();
    let estimated_size_kb: Option<u64> = subkey
        .get_value::<u32, _>("EstimatedSize")
        .ok()
        .map(|v| v as u64);
    let windows_installer = dword_flag(subkey, "WindowsInstaller");
    let system_component = dword_flag(subkey, "SystemComponent");

    let id = format!("{}\\{}\\{}", source.hive_name, source.path, subkey_name);
    let protected = is_protected(&display_name, publisher.as_deref(), system_component);

    Some(InstalledProgram {
        id,
        display_name,
        publisher,
        display_version,
        install_date,
        install_location,
        uninstall_string,
        quiet_uninstall_string,
        estimated_size_kb,
        windows_installer,
        system_component,
        protected,
        hive: source.hive_name.to_string(),
        category: None,
    })
}

fn dword_flag(subkey: &RegKey, name: &str) -> bool {
    subkey
        .get_value::<u32, _>(name)
        .map(|v| v != 0)
        .unwrap_or(false)
}

fn is_protected(display_name: &str, publisher: Option<&str>, system_component: bool) -> bool {
    if system_component {
        return true;
    }

    if APP_DISPLAY_NAMES
        .iter()
        .any(|name| display_name.eq_ignore_ascii_case(name))
    {
        return true;
    }

    let lower = display_name.to_lowercase();
    if lower.contains("kb")
        && (lower.starts_with("update for")
            || lower.starts_with("security update")
            || lower.starts_with("hotfix for"))
    {
        return true;
    }

    let publisher_lower = publisher.unwrap_or("").to_lowercase();
    if publisher_lower.contains("microsoft") {
        if lower.contains("visual c++")
            || lower.contains(".net")
            || lower.contains("windows sdk")
            || lower.contains("microsoft edge update")
            || lower == "microsoft edge"
        {
            return true;
        }
    }

    false
}

pub fn find_program_by_id(id: &str) -> Result<Option<InstalledProgram>, String> {
    let programs = list_installed_programs(true)?;
    Ok(programs.into_iter().find(|p| p.id == id))
}

/// Delete a leftover Uninstall registry subkey identified by `InstalledProgram.id`.
pub fn delete_uninstall_key(id: &str) -> Result<(), String> {
    let (hive_name, rest) = id
        .split_once('\\')
        .ok_or_else(|| format!("Invalid program id: {id}"))?;
    let (parent_path, subkey_name) = rest
        .rsplit_once('\\')
        .ok_or_else(|| format!("Invalid program id path: {id}"))?;

    let root = match hive_name {
        "HKLM" | "HKLM-WOW64" => RegKey::predef(HKEY_LOCAL_MACHINE),
        "HKCU" | "HKCU-WOW64" => RegKey::predef(HKEY_CURRENT_USER),
        _ => return Err(format!("Unknown registry hive in id: {hive_name}")),
    };

    let parent = root
        .open_subkey_with_flags(parent_path, KEY_WRITE)
        .map_err(|e| format!("Cannot open uninstall parent key ({parent_path}): {e}"))?;

    // Prefer delete_subkey_all — some vendors nest values under the uninstall key.
    parent
        .delete_subkey_all(subkey_name)
        .or_else(|_| parent.delete_subkey(subkey_name))
        .map_err(|e| format!("Failed to delete registry key ({subkey_name}): {e}"))?;

    Ok(())
}
