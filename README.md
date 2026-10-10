# brows — Browser Picker

[日本語](README.ja.md)

**Choose which browser opens every link on Windows 11 or Ubuntu.**

A lightweight browser selector that lets you pick Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser each time you click a link — with per-URL rules and Chromium profile support.

[![Latest Release](https://img.shields.io/github/v/release/gentianster/brows)](https://github.com/gentianster/brows/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2011%20%7C%20Linux-blue)](https://github.com/gentianster/brows)

<img width="490" height="557" alt="image" src="https://github.com/user-attachments/assets/ac159a43-d622-4e22-9997-947f5b7f6153" />

<img width="399" height="327" alt="image" src="https://github.com/user-attachments/assets/dff140a7-86d2-4863-9f5a-80fc130ee1fe" />


## About This Project

This app is an experiment in **vibe coding** — letting AI write virtually all of the code and treating the codebase as a black box, with minimal human review. The source is published as OSS as part of that experiment, so anyone can see what comes out of this style of development. Please keep this background in mind when reading the code or using the app.

## Features

- **Browser picker dialog** — choose Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser on every link click
- **Chromium profile support** — select a specific Chrome / Edge / Brave / Vivaldi profile per link
- **URL rules** — automatically open matching URLs in a specific browser without showing the dialog
- **Drag-to-reorder** — rearrange the browser list and it's saved automatically
- **Resident picker** — keeps its window hidden between links; queued links are handled in order
- **Auto-update (Windows)** — checks GitHub Releases once a day; one-click download & restart from the settings screen
- **Native UI** — a single Windows executable or a Linux source build

## Install on Windows

1. Download `brows.exe` from [Releases](https://github.com/gentianster/brows/releases/latest)
2. Place it anywhere (e.g. `C:\Tools\brows.exe`)
3. Double-click `brows.exe` to open the settings screen
4. Click **Register** — a UAC prompt will appear to register brows as a browser handler
5. Go to **Windows Settings → Apps → Default apps → brows** and set it as your default browser

## Build and install on Linux

The initial Linux target is **Ubuntu 26.04 x86_64**, including a Wayland desktop session with **XWayland** available. The picker uses X11 to retain and hide its window between links. Native Wayland is outside this initial scope. Linux binaries are not currently published with the Windows releases.

Build prerequisites on Ubuntu:

```sh
sudo apt install cargo rustc build-essential pkg-config libglib2.0-dev desktop-file-utils
cargo build --release
# → target/release/brows
```

Place the binary at a stable path before registration:

```sh
install -Dm755 target/release/brows "$HOME/.local/bin/brows"
"$HOME/.local/bin/brows" --register
```

Registration creates `$XDG_DATA_HOME/applications/brows.desktop` (default `~/.local/share/applications/brows.desktop`) with HTTP/HTTPS support. It does not change your default browser. Select **brows** in the system's default-app settings, or explicitly run:

```sh
xdg-mime default brows.desktop x-scheme-handler/http
xdg-mime default brows.desktop x-scheme-handler/https
```

Start brows without arguments to open settings. Enable **Start at login** there if wanted; this creates `$XDG_CONFIG_HOME/autostart/brows.desktop` (default `~/.config/autostart/brows.desktop`) pointing to the installed binary with `--resident`. Keep that binary at the registered path, or register again after moving it.

Linux browser discovery and launching use GIO Desktop Entries and preserve their fixed launch arguments. Ordinary native packages of **Vivaldi, Google Chrome, and Chromium** support profile selection. Snap/Flatpak browsers may be launched through their Desktop Entries, but profile selection for those packages is deferred. Linux automatic updates and a tray icon are outside the initial scope.

Candidates are visible HTTP/HTTPS handlers, so registered link-handling apps may also appear. Hidden Desktop Entries are omitted. Browser icons support absolute PNG paths and PNGs in hicolor, XDG icon directories, and pixmaps; SVGs and active-theme inheritance are deferred. Long candidate or expanded profile lists scroll while keeping the picker footer visible.

Before removing brows, choose another default browser and disable its login startup in settings. `brows --unregister` removes its application entry; it does not choose a replacement default browser.

The [Linux design and verification record](docs/linux-support.md) records completed checks and remaining verification limits.

## Usage

Once brows is selected as your default browser, clicking a link in an application will show the browser picker dialog.

### URL Rules

Set up rules so that URLs matching a pattern automatically open in a specific browser — no dialog shown.  
Configure from the **URL Rules** section in the settings screen. Chromium profiles are selectable by display name.

Settings are saved to `%APPDATA%\brows\config.toml` on Windows and `$XDG_CONFIG_HOME/brows/config.toml` (default `~/.config/brows/config.toml`) on Linux.

### Auto-Update on Windows

brows checks GitHub Releases in the background once per day. When a new version is available:

- **Picker screen**: version name shown in the bottom-right
- **Settings screen**: a **Download** button appears — click to update and restart automatically

## Build

Windows requires the [Rust toolchain](https://rustup.rs/). Linux also needs the development packages listed above.

```bash
cargo build --release
# → target/release/brows.exe (Windows), target/release/brows (Linux)
```

Ubuntu's `rustfmt` and `rust-clippy` packages provide the formatting and lint tools. For local validation:

```sh
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo build --release
```

## Requirements

- Windows 11, or Ubuntu 26.04 x86_64 with XWayland and a graphical login session
- Windows registration requires administrator rights; Linux registration is per user
- Linux registration uses `update-desktop-database` from `desktop-file-utils`

## License

[MIT License](LICENSE)
