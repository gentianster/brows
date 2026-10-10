# brows

[English](README.md)

Windows 11 / Ubuntu 向けブラウザ選択ツール。リンクを開くたびに、インストール済みブラウザの一覧からどれで開くか選べます。

[![Latest Release](https://img.shields.io/github/v/release/gentianster/brows)](https://github.com/gentianster/brows/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2011%20%7C%20Linux-blue)](https://github.com/gentianster/brows)

<img width="448" height="415" alt="image" src="https://github.com/user-attachments/assets/b6dc38c2-1266-4bce-9740-96b717296c30" />

<img width="419" height="343" alt="image" src="https://github.com/user-attachments/assets/9be684ce-90f1-45d7-85f9-fb211928b176" />

## このプロジェクトについて

このアプリは **Vibe Coding**（コードをほぼすべて AI に書かせ、中身をブラックボックスとして扱う開発スタイル）のお試しとして作っています。その実験の成果物を誰でも見られるよう、ソースコードを OSS として公開しています。コードを読む際・アプリを使う際は、この背景を踏まえてご覧ください。

## 特徴

- インストール済みブラウザを自動検出（Chrome / Edge / Vivaldi / Brave など）
- Chrome / Edge など Chromium 系ブラウザはプロファイルごとに選択可能
- ブラウザの表示順をドラッグで変更・保存
- URL パターンに応じて自動でブラウザを選択するルールを GUI で設定（設定画面から追加・削除）
- 選択後はウィンドウを非表示にして常駐。連続したリンクは順番に処理
- Windows では1日1回バックグラウンドでアップデートを確認。新バージョンがあればピッカーと設定画面に通知
- Windows では設定画面からワンクリックでダウンロード＆再起動

## Windows へのインストール

1. [Releases](https://github.com/gentianster/brows/releases/latest) から `brows.exe` をダウンロード
2. 任意のフォルダに配置
3. `brows.exe` をダブルクリックして設定画面を開く
4. **「登録」** をクリック（UAC プロンプトが表示されます）
5. Windows 設定 → アプリ → 既定のアプリ → **brows** を既定のブラウザに設定

## Linux のビルドとインストール

初期対応は **Ubuntu 26.04 x86_64** を対象とし、Wayland セッションでは **XWayland** を利用します。ウィンドウを保持して非表示・再表示するため、ピッカーは X11 バックエンドで動きます。ネイティブ Wayland は初期対応の範囲外です。Linux のビルド済みバイナリは現時点で公開していません。

Ubuntu のビルド用パッケージ:

```sh
sudo apt install cargo rustc build-essential pkg-config libglib2.0-dev desktop-file-utils
cargo build --release
# → target/release/brows
```

登録前に、バイナリを継続して使う場所へ配置します。

```sh
install -Dm755 target/release/brows "$HOME/.local/bin/brows"
"$HOME/.local/bin/brows" --register
```

登録先は `$XDG_DATA_HOME/applications/brows.desktop`（既定は `~/.local/share/applications/brows.desktop`）です。HTTP/HTTPS 対応アプリとして登録しますが、既定ブラウザは変更しません。システムの既定アプリ設定から **brows** を選ぶか、変更したい場合に次を明示実行します。

```sh
xdg-mime default brows.desktop x-scheme-handler/http
xdg-mime default brows.desktop x-scheme-handler/https
```

引数なし起動で設定画面を開けます。起動時常駐を有効にすると `$XDG_CONFIG_HOME/autostart/brows.desktop`（既定は `~/.config/autostart/brows.desktop`）へ、配置したバイナリを `--resident` で起動する設定を保存します。登録後は同じ場所にバイナリを置き、移動した場合は再登録してください。

Linux のブラウザ検出・起動には GIO の Desktop Entry を使い、元の固定引数を維持します。通常パッケージの **Vivaldi / Google Chrome / Chromium** ではプロファイルを選択できます。Snap/Flatpak は Desktop Entry から通常起動できますが、これらのプロファイル選択は後続対応です。Linux の自動更新とトレイアイコンは初期対応の範囲外です。

候補は表示可能な HTTP/HTTPS ハンドラーなので、リンクを扱うアプリも表示されます。非表示の Desktop Entry は除外します。ブラウザ画像アイコンは絶対パスの PNG と hicolor・XDG のアイコンディレクトリ・pixmaps の PNG に対応し、SVG と使用中テーマの継承は後続対応です。候補や展開したプロファイルが多い場合は一覧をスクロールでき、下部のボタンは画面内に残ります。

`brows --quit` で常駐を終了できます。削除前に別の既定ブラウザを選び、設定画面で起動時常駐を無効にしてください。`brows --unregister` はアプリ登録を解除しますが、代わりの既定ブラウザは選びません。

実施済みの確認と残る検証範囲は [Linux 設計・検証記録](docs/linux-support.md) に記載します。

## 使い方

brows を既定ブラウザに設定すると、任意のアプリからリンクを開いたときにブラウザ選択ダイアログが表示されます。

### URL ルール設定

設定画面の「URL ルール」セクションから、URL パターンに応じたブラウザの自動選択を設定できます。パターンにマッチした URL はブラウザ選択ダイアログを表示せず、直接指定のブラウザで開きます。

Chrome などのプロファイルも選択可能です（プロファイルの表示名で指定）。

設定は Windows では `%APPDATA%\brows\config.toml`、Linux では `$XDG_CONFIG_HOME/brows/config.toml`（既定は `~/.config/brows/config.toml`）に保存されます。「設定ファイルを開く」ボタンから直接編集することもできます。

### Windows のアップデート

起動時にバックグラウンドで GitHub Releases を確認します（1日1回）。新バージョンがある場合：

- **ピッカー画面**：右下にバージョン名を表示
- **設定画面**：「ダウンロード」ボタンが表示され、クリックで自動更新＆再起動

## ビルド

```bash
cargo build --release
```

Windows では `target/release/brows.exe`、Linux では `target/release/brows` が生成されます。Windows は Rust ツールチェイン、Linux は上記の開発用パッケージも必要です。

fmt / clippy の検証には `sudo apt install rustfmt rust-clippy` も必要です。ローカルでの検証コマンド:

```sh
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo build --release
```

## 要件

- Windows 11、または XWayland を利用できる Ubuntu 26.04 x86_64 の GUI セッション
- Windows の登録には管理者権限が必要。Linux はユーザー単位で登録
- Linux の登録には `desktop-file-utils` の `update-desktop-database` を利用

## ライセンス

[MIT License](LICENSE)
