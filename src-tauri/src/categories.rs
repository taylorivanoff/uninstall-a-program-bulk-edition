use crate::programs::InstalledProgram;

/// Fill missing categories from display-name heuristics.
pub fn enrich_categories(programs: &mut [InstalledProgram]) {
    for program in programs.iter_mut() {
        if program
            .category
            .as_ref()
            .is_some_and(|c| !c.trim().is_empty())
        {
            continue;
        }
        program.category = Some(infer_category(program).to_string());
    }
}

pub fn infer_category(program: &InstalledProgram) -> &'static str {
    let name = program.display_name.to_ascii_lowercase();
    let publisher = program
        .publisher
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let compact: String = format!("{name}{publisher}")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();

    if matches_any(
        &compact,
        &[
            "chrome",
            "firefox",
            "edge",
            "brave",
            "opera",
            "torbrowser",
            "vivaldi",
            "waterfox",
        ],
    ) {
        return "Browsers";
    }
    if matches_any(
        &compact,
        &[
            "7zip", "winrar", "peazip", "everything", "treesize", "bandizip",
        ],
    ) {
        return "Compression";
    }
    if matches_any(
        &compact,
        &[
            "vlc",
            "spotify",
            "itunes",
            "audacity",
            "obsstudio",
            "obs studio",
            "handbrake",
            "mpchc",
            "foobar2000",
            "kodi",
            "plex",
            "aimp",
            "potplayer",
        ],
    ) {
        return "Media";
    }
    if matches_any(
        &compact,
        &[
            "paintdotnet",
            "paintnet",
            "gimp",
            "inkscape",
            "sharex",
            "lightshot",
            "greenshot",
            "irfanview",
            "xnview",
            "blender",
        ],
    ) {
        return "Imaging";
    }
    if matches_any(
        &compact,
        &[
            "acrobat",
            "adobereader",
            "sumatrapdf",
            "libreoffice",
            "openoffice",
            "onlyoffice",
            "foxit",
            "cutepdf",
        ],
    ) {
        return "Documents";
    }
    if matches_any(
        &compact,
        &[
            "discord",
            "slack",
            "zoom",
            "teams",
            "telegram",
            "whatsapp",
            "signal",
            "skype",
            "thunderbird",
            "mailspring",
        ],
    ) {
        return "Communication";
    }
    if matches_any(
        &compact,
        &[
            "steam",
            "epicgames",
            "goggalaxy",
            "origin",
            "eaapp",
            "ubisoft",
            "battle",
            "minecraft",
        ],
    ) {
        return "Gaming";
    }
    if matches_any(
        &compact,
        &[
            "git",
            "github",
            "vscode",
            "visualstudiocode",
            "visual studio code",
            "notepadplusplus",
            "sublime",
            "jetbrains",
            "nodejs",
            "python",
            "rustup",
            "golang",
            "docker",
            "postman",
            "insomnia",
            "windowsterminal",
            "windows terminal",
            "powershell",
            "pwsh",
            "powertoys",
            "wsl",
            "ohmyposh",
            "winscp",
            "putty",
            "filezilla",
            "wireshark",
            "sysinternals",
            "cmake",
            "ninja",
            "yarn",
            "pnpm",
            "neovim",
            "vim",
            "emacs",
            "androidstudio",
            "intellij",
            "pycharm",
            "webstorm",
            "cursor",
        ],
    ) {
        return "Development";
    }
    if matches_any(
        &compact,
        &[
            "malwarebytes",
            "bitwarden",
            "1password",
            "keepass",
            "lastpass",
            "nordvpn",
            "expressvpn",
            "authy",
            "verity",
            "clamav",
        ],
    ) {
        return "Security";
    }
    if matches_any(
        &compact,
        &[
            "dropbox",
            "googledrive",
            "onedrive",
            "notion",
            "obsidian",
            "onenote",
            "evernote",
            "megasync",
            "nextcloud",
        ],
    ) {
        return "Cloud";
    }
    if matches_any(
        &compact,
        &[
            "dotnet",
            "temurin",
            "openjdk",
            "jre",
            "jdk",
            "vcredist",
            "visualc",
            "directx",
        ],
    ) {
        return "Runtimes";
    }
    if matches_any(
        &compact,
        &[
            "cpu-z",
            "cpuz",
            "hwmonitor",
            "crystaldisk",
            "qbittorrent",
            "etcher",
            "rufus",
            "ccleaner",
            "revo",
            "speccy",
            "hwinfo",
        ],
    ) {
        return "Utilities";
    }

    "Other"
}

fn matches_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| {
        let needle: String = n
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        !needle.is_empty() && hay.contains(&needle)
    })
}
