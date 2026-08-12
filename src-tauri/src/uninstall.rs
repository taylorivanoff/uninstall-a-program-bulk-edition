use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::programs::{delete_uninstall_key, find_program_by_id, InstalledProgram};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallProgress {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub message: Option<String>,
    pub exit_code: Option<i32>,
}

enum RunOutcome {
    Exit(i32),
    MissingExecutable(String),
}

pub fn uninstall_programs(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Err("No programs selected".into());
    }

    for id in &ids {
        let Some(program) = find_program_by_id(id)? else {
            emit_progress(
                &app,
                id,
                "(unknown)",
                "failed",
                Some("Program no longer found in registry".into()),
                None,
            );
            continue;
        };

        if program.protected {
            emit_progress(
                &app,
                &program.id,
                &program.display_name,
                "failed",
                Some("Protected entry — skipped".into()),
                None,
            );
            continue;
        }

        emit_progress(
            &app,
            &program.id,
            &program.display_name,
            "running",
            Some("Starting silent uninstall…".into()),
            None,
        );

        match run_uninstaller(&program) {
            Ok(RunOutcome::Exit(code)) if code == 0 || code == 3010 => {
                let msg = if code == 3010 {
                    Some("Uninstalled (reboot required)".into())
                } else {
                    Some("Uninstalled successfully".into())
                };
                emit_progress(
                    &app,
                    &program.id,
                    &program.display_name,
                    "uninstalled",
                    msg,
                    Some(code),
                );
            }
            Ok(RunOutcome::Exit(code)) => {
                emit_progress(
                    &app,
                    &program.id,
                    &program.display_name,
                    "failed",
                    Some(format!("Uninstaller exited with code {code}")),
                    Some(code),
                );
            }
            Ok(RunOutcome::MissingExecutable(path)) => {
                cleanup_missing_uninstaller(&app, &program, &path);
            }
            Err(err) => {
                emit_progress(
                    &app,
                    &program.id,
                    &program.display_name,
                    "failed",
                    Some(err),
                    None,
                );
            }
        }

        std::thread::sleep(Duration::from_millis(400));
    }

    let _ = app.emit("uninstall-finished", ());
    Ok(())
}

fn cleanup_missing_uninstaller(app: &AppHandle, program: &InstalledProgram, path: &str) {
    emit_progress(
        app,
        &program.id,
        &program.display_name,
        "running",
        Some(format!(
            "Uninstaller not found ({path}) — removing leftover registry entry…"
        )),
        None,
    );

    match delete_uninstall_key(&program.id) {
        Ok(()) => {
            emit_progress(
                app,
                &program.id,
                &program.display_name,
                "uninstalled",
                Some("Uninstaller missing — removed leftover registry entry".into()),
                Some(0),
            );
        }
        Err(err) => {
            emit_progress(
                app,
                &program.id,
                &program.display_name,
                "failed",
                Some(format!(
                    "Uninstaller missing ({path}) and registry cleanup failed: {err}"
                )),
                None,
            );
        }
    }
}

fn emit_progress(
    app: &AppHandle,
    id: &str,
    display_name: &str,
    status: &str,
    message: Option<String>,
    exit_code: Option<i32>,
) {
    let _ = app.emit(
        "uninstall-progress",
        UninstallProgress {
            id: id.to_string(),
            display_name: display_name.to_string(),
            status: status.to_string(),
            message,
            exit_code,
        },
    );
}

fn run_uninstaller(program: &InstalledProgram) -> Result<RunOutcome, String> {
    // Prefer QuietUninstallString — already intended to be silent.
    if let Some(quiet) = program
        .quiet_uninstall_string
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return spawn_command_line(quiet, SilentMode::AlreadyQuiet);
    }

    if program.windows_installer {
        if let Some(guid) = extract_product_code(program) {
            return run_msiexec_uninstall(&guid);
        }
    }

    if let Some(uninstall) = program
        .uninstall_string
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Some(guid) = msiexec_guid_from_string(uninstall) {
            return run_msiexec_uninstall(&guid);
        }
        // Unity Hub/Editor and most NSIS uninstallers need /S for silent removal.
        return spawn_command_line(uninstall, SilentMode::EnsureSilent);
    }

    Err("No uninstall command available".into())
}

fn extract_product_code(program: &InstalledProgram) -> Option<String> {
    if let Some(name) = program.id.rsplit('\\').next() {
        if looks_like_guid(name) {
            return Some(normalize_guid(name));
        }
    }
    program
        .uninstall_string
        .as_deref()
        .and_then(msiexec_guid_from_string)
}

fn msiexec_guid_from_string(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    if !lower.contains("msiexec") {
        return None;
    }
    let start = s.find('{')?;
    let end = s[start..].find('}')? + start;
    let guid = &s[start..=end];
    if looks_like_guid(guid) {
        Some(normalize_guid(guid))
    } else {
        None
    }
}

fn looks_like_guid(s: &str) -> bool {
    let s = s.trim();
    s.len() == 38 && s.starts_with('{') && s.ends_with('}')
}

fn normalize_guid(s: &str) -> String {
    s.trim().to_uppercase()
}

fn run_msiexec_uninstall(product_code: &str) -> Result<RunOutcome, String> {
    let output = Command::new("msiexec.exe")
        .args(["/x", product_code, "/qn", "/norestart"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to start msiexec: {e}"))?;

    Ok(RunOutcome::Exit(output.status.code().unwrap_or(-1)))
}

fn spawn_command_line(command_line: &str, silent: SilentMode) -> Result<RunOutcome, String> {
    let (program, mut args) = split_command_line(command_line)
        .ok_or_else(|| format!("Could not parse uninstall command: {command_line}"))?;

    if matches!(silent, SilentMode::EnsureSilent) {
        ensure_silent_args(&program, &mut args);
    }

    if looks_like_filesystem_path(&program) && !Path::new(&program).is_file() {
        return Ok(RunOutcome::MissingExecutable(program));
    }

    let mut cmd = Command::new(&program);
    cmd.args(&args);
    // Hide console flicker for silent runs; keep normal for interactive fallbacks.
    if args_look_silent(&args) || matches!(silent, SilentMode::AlreadyQuiet) {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.status() {
        Ok(status) => Ok(RunOutcome::Exit(status.code().unwrap_or(-1))),
        Err(err) if is_file_not_found(&err) => Ok(RunOutcome::MissingExecutable(program)),
        Err(err) => Err(format!("Failed to start uninstaller ({program}): {err}")),
    }
}

#[derive(Clone, Copy)]
enum SilentMode {
    /// QuietUninstallString — trust registry args as-is.
    AlreadyQuiet,
    /// UninstallString — append common silent switches when missing.
    EnsureSilent,
}

fn args_look_silent(args: &[String]) -> bool {
    args.iter().any(|a| is_silent_flag(a))
}

fn is_silent_flag(arg: &str) -> bool {
    let u = arg.trim().to_ascii_uppercase();
    matches!(
        u.as_str(),
        "/S" | "-S"
            | "/SILENT"
            | "/VERYSILENT"
            | "/QUIET"
            | "/QUIET="
            | "/QN"
            | "/QB"
            | "/Q"
            | "--SILENT"
            | "/SUPPRESSMSGBOXES"
    ) || u.starts_with("/SILENT")
        || u.starts_with("/VERYSILENT")
        || u.starts_with("/QUIET")
}

fn ensure_silent_args(program: &str, args: &mut Vec<String>) {
    if args_look_silent(args) {
        return;
    }

    let name = Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Inno Setup: unins000.exe, unins001.exe, …
    if is_inno_uninstaller(&name) {
        args.extend(
            ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
                .into_iter()
                .map(str::to_string),
        );
        return;
    }

    // NSIS / Unity Hub / Unity Editor Uninstall*.exe — silent switch is /S
    if name.contains("uninstall") || name == "uninst.exe" || name.ends_with(".exe") {
        args.push("/S".into());
    }
}

fn is_inno_uninstaller(file_name: &str) -> bool {
    let stem = file_name.strip_suffix(".exe").unwrap_or(file_name);
    match stem.strip_prefix("unins") {
        Some(rest) if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) => true,
        _ => false,
    }
}

fn looks_like_filesystem_path(program: &str) -> bool {
    program.contains('\\')
        || program.contains('/')
        || Path::new(program).is_absolute()
}

fn is_file_not_found(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::InvalidFilename
    ) || err.raw_os_error() == Some(2)
        || err.raw_os_error() == Some(3)
}

/// Split a Windows-style command line into program + args.
/// Handles quoted paths and common unquoted `C:\Program Files\...\app.exe /S` forms.
fn split_command_line(input: &str) -> Option<(String, Vec<String>)> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if input.starts_with('"') {
        return split_quoted_command_line(input);
    }

    if let Some((program, rest)) = split_unquoted_exe_path(input) {
        let args = if rest.is_empty() {
            Vec::new()
        } else {
            split_arg_tokens(rest)
        };
        return Some((program, args));
    }

    // Fallback: first whitespace-separated token.
    let mut parts = split_arg_tokens(input);
    if parts.is_empty() {
        return None;
    }
    let program = parts.remove(0);
    Some((program, parts))
}

fn split_quoted_command_line(input: &str) -> Option<(String, Vec<String>)> {
    let mut chars = input.chars().peekable();
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        return None;
    }
    let program = parts.remove(0);
    Some((program, parts))
}

fn split_unquoted_exe_path(input: &str) -> Option<(String, &str)> {
    let lower = input.to_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(idx) = lower.find(ext) {
            let end = idx + ext.len();
            let program = input[..end].to_string();
            let rest = input[end..].trim_start();
            return Some((program, rest));
        }
    }
    None
}

fn split_arg_tokens(input: &str) -> Vec<String> {
    let mut chars = input.chars().peekable();
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_path() {
        let (prog, args) =
            split_command_line(r#""C:\Program Files\App\uninstall.exe" /S /foo"#).unwrap();
        assert_eq!(prog, r"C:\Program Files\App\uninstall.exe");
        assert_eq!(args, vec!["/S", "/foo"]);
    }

    #[test]
    fn splits_unquoted_program_files_path() {
        let (prog, args) = split_command_line(
            r"C:\Program Files\Unity\Hub\Editor\6000.2.15f1\Editor\Uninstall.exe /S",
        )
        .unwrap();
        assert_eq!(
            prog,
            r"C:\Program Files\Unity\Hub\Editor\6000.2.15f1\Editor\Uninstall.exe"
        );
        assert_eq!(args, vec!["/S"]);
    }

    #[test]
    fn extracts_msiexec_guid() {
        let guid = msiexec_guid_from_string(
            r"MsiExec.exe /X{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}",
        )
        .unwrap();
        assert_eq!(guid, "{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}");
    }

    #[test]
    fn adds_nsis_silent_flag_when_missing() {
        let mut args = vec![];
        ensure_silent_args(r"C:\Program Files\Unity Hub\Uninstall Unity Hub.exe", &mut args);
        assert_eq!(args, vec!["/S"]);
    }

    #[test]
    fn does_not_duplicate_silent_flag() {
        let mut args = vec!["/S".into()];
        ensure_silent_args(r"C:\Program Files\App\Uninstall.exe", &mut args);
        assert_eq!(args, vec!["/S"]);
    }
}
