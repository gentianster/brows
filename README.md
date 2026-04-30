# brows — Windows 11 Browser Picker

**Choose which browser opens every link on Windows 11.**  
A lightweight browser selector that lets you pick Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser each time you click a link — with per-URL rules and Chromium profile support.

[![Latest Release](https://img.shields.io/github/v/release/gentianster/brows)](https://github.com/gentianster/brows/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-blue)](https://github.com/gentianster/brows)

> Windows 11 向けブラウザ選択ツール。リンクを開くたびに、インストール済みブラウザの一覧からどれで開くか選べます。

<img width="448" height="415" alt="image" src="https://github.com/user-attachments/assets/b6dc38c2-1266-4bce-9740-96b717296c30" />

<img width="419" height="343" alt="image" src="https://github.com/user-attachments/assets/9be684ce-90f1-45d7-85f9-fb211928b176" />

## Features / 特徴

- **Browser picker dialog** — choose Chrome, Edge, Brave, Vivaldi, Firefox, or any installed browser on every link click
- **Chromium profile support** — select a specific Chrome / Edge / Brave / Vivaldi profile per link
- **URL rules** — automatically open matching URLs in a specific browser without showing the dialog
- **Drag-to-reorder** — rearrange browser list order and it's saved automatically
- **Auto-update** — checks GitHub Releases once a day; one-click download & restart from the settings screen
- **Tiny & native** — single `.exe`, no installer, no runtime dependencies

---

- インストール済みブラウザを自動検出（Chrome / Edge / Vivaldi / Brave など）
- Chrome / Edge など Chromium 系ブラウザはプロファイルごとに選択可能
- ブラウザの表示順をドラッグで変更・保存
- URL パターンに応じて自動でブラウザを選択するルールを GUI で設定
- 1日1回バックグラウンドでアップデートを確認。新バージョンがあればピッカーと設定画面に通知
- 設定画面からワンクリックでダウンロード＆再起動

## Install / インストール

1. Download `brows.exe` from [Releases](https://github.com/gentianster/brows/releases/latest)
2. Place it anywhere (e.g. `C:\Tools\brows.exe`)
3. Double-click `brows.exe` to open the settings screen
4. Click **「登録」** — a UAC prompt will appear to register brows as a browser handler
5. Go to **Windows Settings → Apps → Default apps → brows** and set it as your default browser

---

1. [Releases](https://github.com/gentianster/brows/releases/latest) から `brows.exe` をダウンロード
2. 任意のフォルダに配置
3. `brows.exe` をダブルクリックして設定画面を開く
4. **「登録」** をクリック（UAC プロンプトが表示されます）
5. Windows 設定 → アプリ → 既定のアプリ → **brows** を既定のブラウザに設定

## Usage / 使い方

Once registered, clicking any link in any application will show the browser picker dialog.

登録後は、任意のアプリからリンクを開くと自動的にブラウザ選択ダイアログが表示されます。

### URL Rules / URL ルール設定

Set up rules so that URLs matching a pattern automatically open in a specific browser — no dialog shown.  
Configure from the **「URL ルール」** section in the settings screen. Chromium profiles are selectable by display name.

URLパターンにマッチしたリンクはダイアログを表示せず、直接指定のブラウザで開きます。設定画面の「URL ルール」セクションから追加・削除できます。

Settings are saved to `%APPDATA%\brows\config.toml`.

### Auto-Update / アップデート

brows checks GitHub Releases in the background once per day. When a new version is available:

- **Picker screen**: version name shown in the bottom-right
- **Settings screen**: a **「ダウンロード」** button appears — click to update and restart automatically

## Build / ビルド

Requires only the [Rust toolchain](https://rustup.rs/). No external tools needed.

```bash
cargo build --release
# → target/release/brows.exe
```

## Requirements / 要件

- Windows 11
- Administrator rights required for registering as default browser handler

## License / ライセンス

[MIT License](LICENSE)
