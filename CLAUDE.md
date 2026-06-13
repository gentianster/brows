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
├── browser.rs    ブラウザ検出（レジストリ + Chrome プロファイル）、バックグラウンド再検出
├── icon.rs       Windows Shell API でブラウザアイコンを取得
├── ipc.rs        常駐インスタンスへの URL 転送（ループバック TCP 127.0.0.1:48693）
├── registry.rs   Windows レジストリへの登録・解除
├── config.rs     設定ファイル（TOML）の読み書き
├── lang.rs       UI 文言（日本語/英語）
├── updater.rs    GitHub Releases からの自動更新
├── util.rs       共有ヘルパー（json_str・detached spawn・プロセス作成フラグ）
└── ui/
    ├── mod.rs      共通セットアップ（アプリアイコン・日本語フォント）
    ├── picker.rs   ブラウザ選択ピッカー（常駐・IPC サーバー含む）
    ├── settings.rs 設定画面（登録・URL ルール・更新）
    └── win32.rs    egui で扱えないウィンドウ操作（再表示・非表示・中央配置）
```

設定（config.toml）への書き込みは `Config::update` 経由で行うこと。
in-memory の `Config` を直接 `save()` すると、バックグラウンドスレッド
（更新チェック・ブラウザ再検出）の書き込みと競合してフィールドを巻き戻す。

## 常駐動作

ピッカーは初回起動後プロセスを終了せず、ウィンドウを非表示にして常駐する。
2 回目以降の起動は常駐インスタンスへ URL を転送して即終了するため、ウィンドウ表示が高速。

- シングルインスタンス判定はポート 48693 の bind 成否で行う（bind 成功 = 常駐になる）
- `--resident` でウィンドウを表示せず常駐だけ開始できる（設定画面の「Windows 起動時に常駐する」で HKCU の Run キーに登録）
- 非表示ウィンドウは再描画イベントを受け取れないため、再表示は egui のコマンドではなく Win32 API（`ShowWindow`）で直接行う
- 自動更新の再起動前に `BROWS-EXIT` を送って常駐を終了させる（古い exe の常駐が残らないように）

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
- ピッカーは選択後もタスクマネージャーに brows.exe が残る（常駐仕様）
- `windows_subsystem = "windows"` を指定しているのでコンソール出力は不可
- Chromium 系以外のブラウザ（Firefox 等）はプロファイル展開非対応
- 日本語フォントは `YuGothM.ttc` → `meiryo.ttc` → `msgothic.ttc` の順でフォールバック
