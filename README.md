# Uninstall Many Programs

[![Release](https://img.shields.io/github/v/release/taylorivanoff/uninstall-a-program-bulk-edition)](https://github.com/taylorivanoff/uninstall-a-program-bulk-edition/releases)
[![Downloads](https://img.shields.io/github/downloads/taylorivanoff/uninstall-a-program-bulk-edition/total)](https://github.com/taylorivanoff/uninstall-a-program-bulk-edition/releases)
![License](https://img.shields.io/badge/license-proprietary-lightgrey)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6)
![Stack](https://img.shields.io/badge/stack-Tauri%202%20%2B%20Rust-24C8DB)

Windows Settings and Control Panel force you to remove apps one at a time. Uninstall many programs at once on Windows. Search, multi-select, and remove Win32/MSI apps in one run.

<img width="1155" height="796" alt="{43BF515F-4566-4AF2-8EC9-BAE97DC0009F}" src="https://github.com/user-attachments/assets/3e924e9d-b9d7-4928-8417-58cfa8c3c40f" />

## Features

- **List all installed programs** from the Windows Uninstall registry (64-bit + 32-bit)
- **Search, sort, and multi-select** apps by name, publisher, version, or size
- **Uninstall many programs in one run** — sequentially, with live status and a clear activity log
- Prefer **silent / quiet uninstall** when the vendor provides `QuietUninstallString`
- Fall back to **MSI** (`msiexec /x`) or the standard uninstall command
- **Protect** itself, system components, and common Microsoft runtimes from accidental removal
- Run with **administrator elevation** so machine-wide uninstalls actually work

## Installation

1. Download the latest installer from [Releases](https://github.com/taylorivanoff/uninstall-a-program-bulk-edition/releases)
2. Run the setup exe and follow the prompts (WebView2 Runtime is used if already installed)

## Build from source

```bash
bun install
bun run build
```

Installer output:

`src-tauri/target/release/bundle/nsis/Uninstall Many Programs_0.1.0_x64-setup.exe`

## Develop

Requires Rust (MSVC), WebView2, and Bun/npm. Sibling crate [`tauri-tray-base`](https://github.com/taylorivanoff/tauri-tray-base) must sit at `../tauri-tray-base` relative to this repo (i.e. `Projects/tauri-tray-base`).

```bash
bun install
bun start      # or: bun run dev
bun run build
bun run release
bun run bump   # patch version across package.json / Cargo.toml / tauri.conf.json
```

## Keywords

`uninstall many programs` · `uninstall a program` · `uninstall programs` · `windows uninstaller` · `bulk uninstall` · `uninstall multiple programs` · `batch remove software` · `windows program remover` · `msi quiet uninstall` · `pc cleanup` · `software uninstaller windows 11`

## License

Proprietary — all rights reserved unless otherwise stated by the author.
