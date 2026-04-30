# brows — Windows 11 Browser Picker

[日本語](README.ja.md)

**Choose which browser opens every link on Windows 11.**  
A lightweight browser selector that lets you pick Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser each time you click a link — with per-URL rules and Chromium profile support.

[![Latest Release](https://img.shields.io/github/v/release/gentianster/brows)](https://github.com/gentianster/brows/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-blue)](https://github.com/gentianster/brows)

<img width="490" height="557" alt="image" src="https://github.com/user-attachments/assets/ac159a43-d622-4e22-9997-947f5b7f6153" />

<img width="399" height="327" alt="image" src="https://github.com/user-attachments/assets/dff140a7-86d2-4863-9f5a-80fc130ee1fe" />


## Features

- **Browser picker dialog** — choose Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser on every link click
- **Chromium profile support** — select a specific Chrome / Edge / Brave / Vivaldi profile per link
- **URL rules** — automatically open matching URLs in a specific browser without showing the dialog
- **Drag-to-reorder** — rearrange the browser list and it's saved automatically
- **Auto-update** — checks GitHub Releases once a day; one-click download & restart from the settings screen
- **Tiny & native** — single `.exe`, no installer, no runtime dependencies

## Install

1. Download `brows.exe` from [Releases](https://github.com/gentianster/brows/releases/latest)
2. Place it anywhere (e.g. `C:\Tools\brows.exe`)
3. Double-click `brows.exe` to open the settings screen
4. Click **Register** — a UAC prompt will appear to register brows as a browser handler
5. Go to **Windows Settings → Apps → Default apps → brows** and set it as your default browser

## Usage

Once registered, clicking any link in any application will show the browser picker dialog.

### URL Rules

Set up rules so that URLs matching a pattern automatically open in a specific browser — no dialog shown.  
Configure from the **URL Rules** section in the settings screen. Chromium profiles are selectable by display name.

Settings are saved to `%APPDATA%\brows\config.toml`.

### Auto-Update

brows checks GitHub Releases in the background once per day. When a new version is available:

- **Picker screen**: version name shown in the bottom-right
- **Settings screen**: a **Download** button appears — click to update and restart automatically

## Build

Requires only the [Rust toolchain](https://rustup.rs/). No external tools needed.

```bash
cargo build --release
# → target/release/brows.exe
```

## Requirements

- Windows 11
- Administrator rights required for registering as default browser handler

## License

[MIT License](LICENSE)
