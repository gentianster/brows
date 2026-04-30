# brows

Windows 11 向けブラウザ選択ツール。任意のアプリからリンクを開く際に、インストール済みブラウザの一覧から選択できる。

## 概要

`brows.exe` を既定ブラウザとして登録しておくと、リンクを開くたびにブラウザ選択ダイアログが表示される。Chrome などの Chromium 系ブラウザはプロファイルごとに選択可能。

## ビルド・実行

```bash
# デバッグビルド＆動作確認
cargo run -- https://example.com   # ピッカー UI を起動
cargo run -- --list                # 検出済みブラウザ一覧

# リリースビルド
cargo build --release
# → target/release/brows.exe を任意の場所に配置する
```

## インストール手順

1. `cargo build --release` でビルド
2. 任意の場所に `brows.exe` を配置
3. **管理者権限**で `brows.exe --register` を実行
4. Windows 設定 → アプリ → 既定のアプリ → brows を既定ブラウザに設定

## プロジェクト構成

```
src/
├── main.rs       エントリーポイント（サブコマンド振り分け）
├── browser.rs    ブラウザ検出（レジストリ + Chrome プロファイル）
├── icon.rs       Windows Shell API でブラウザアイコンを取得
├── registry.rs   Windows レジストリへの登録・解除
├── config.rs     設定ファイル（TOML）の読み書き
└── ui.rs         egui による UI（ピッカー・設定画面）
```

## 設定ファイル

`%APPDATA%\brows\config.toml` に保存される。

```toml
browser_order = ["C:\\...\\chrome.exe", "C:\\...\\msedge.exe"]

[default_browser]
# 自動で開くブラウザ名

[[rules]]
pattern = "github.com"
browser = "Google Chrome"
```

## Git ワークフロー

**必ず feature ブランチを切ってから作業する。**

```bash
git checkout -b feature/xxx   # 作業前に切る
# ... 実装 ...
git add <files>
git commit -m "..."
git push -u origin feature/xxx
gh pr create
```

`main` への直接 push はしない。

## 既知の注意点

- `--register` / `--unregister` は管理者権限が必要
- `windows_subsystem = "windows"` を指定しているのでコンソール出力は不可
- Chromium 系以外のブラウザ（Firefox 等）はプロファイル展開非対応
- 日本語フォントは `YuGothM.ttc` → `meiryo.ttc` → `msgothic.ttc` の順でフォールバック
