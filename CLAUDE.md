# brows

任意のアプリから開く HTTP/HTTPS リンクのブラウザを選択する Rust / eframe アプリ。
Windows 11 の既存実装に加え、Ubuntu 26.04 x86_64 の XWayland 向け実装を持つ。
Linux の設計、完了条件、実施済み・未実施の確認は
[docs/linux-support.md](docs/linux-support.md) を参照する。実装の存在だけで
Linux / Windows の実機動作や性能を検証済みと扱わない。

## 対応範囲

- Windows 11: レジストリ登録、名前付きパイプ、トレイ、Shell アイコン、自動更新。
- Linux: Wayland セッションの XWayland 上で X11 / glow を使用。
  常駐プロセスとピッカーウィンドウを保持し、選択・キャンセル後に非表示、次の URL で再表示する。
- Linux のブラウザ検出と URI 起動には GIO の Desktop Entry を使う。
  Desktop Entry ID と Exec の固定引数を保持し、brows 自身と非表示 Entry は候補から除外する。
  候補は表示可能な HTTP/HTTPS ハンドラーで、WebBrowser カテゴリーには限定しない。
- Linux の画像アイコンは Desktop Entry の絶対 PNG または hicolor / XDG icons / pixmaps の PNG を読む。
  多い候補・展開プロファイルは一覧内でスクロールし、下部の操作を表示する。
- Linux のプロファイル選択は通常パッケージの Vivaldi / Google Chrome / Chromium が対象。
  Local State のプロファイル名とディレクトリの存在を使い、履歴・Cookie・資格情報は読まない。
  起動時は元のランチャー引数と `--profile-directory` を維持する。
- Linux ネイティブ Wayland、Snap/Flatpak のプロファイル選択、自動更新、
  トレイ、SVG アイコンと使用中テーマの継承、Linux 配布物の公開は初期対応の範囲外。
  設定画面は引数なし起動で開く。

## ビルド・検証

Rust ツールチェインを使い、Linux / Windows それぞれのホストで native build を確認する。
Linux の開発用パッケージと配置手順は README.md / README.ja.md を参照する。
Linux ホストからの Windows cross compile は初期対応の完了条件に含めない。
Windows の resource 生成は Windows ホストを対象とする。

```sh
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo build --release
# Windows: target/release/brows.exe
# Linux:   target/release/brows
```

テストは合成 Desktop Entry、Local State、捕捉ランチャー、一時ディレクトリを使用する。
Linux の GUI 動作確認も XDG_CONFIG_HOME / XDG_DATA_HOME / XDG_RUNTIME_DIR を隔離し、
仮ブラウザだけを候補にする。`--list` はレジストリ / Desktop Entry の候補を列挙する。
ピッカーと設定画面のグループ検出は Local State を読むため、実プロファイルを読む承認がない
調査・試験では、GUI を合成環境だけで起動する。
既存ブラウザの操作、依存インストール、実ユーザーの既定 MIME / autostart の変更は、
その作業についてユーザーの承認がある場合に実行する。

合成環境で使用する起動引数:

```sh
cargo run -- https://example.invalid/  # ピッカーへ HTTP/HTTPS URL を一つ渡す
cargo run -- --list                   # 検出したブラウザ一覧
cargo run -- --resident               # 非表示のまま常駐
cargo run -- --quit                   # 同じユーザーの常駐プロセスを終了
cargo run                            # 設定画面
```

GUI の確認は初回表示、非表示後の同じウィンドウの再表示、フォーカス、日本語、
連続 URL の選択・キャンセル、ルールの直接起動、設定画面を含める。
fmt / clippy / tests / build と GUI の結果を分けて記録する。
未実行の確認、失敗、残るブロッカーを明記し、速度は測定してから述べる。

## 登録・起動時常駐

バイナリを継続して使う場所へ配置してから登録する。

- Windows の `--register` / `--unregister` は管理者権限が必要。
  登録後に Windows 設定 → アプリ → 既定のアプリ → brows から既定ブラウザを選ぶ。
  起動時常駐は HKCU の Run キーに `--resident` を登録する。
- Linux の `--register` / `--unregister` はユーザー単位の
  `$XDG_DATA_HOME/applications/brows.desktop` を登録・解除する。
  既定の配置先は `~/.local/share/applications/brows.desktop`。
  HTTP/HTTPS の MimeType を記載し、`desktop-file-utils` の `update-desktop-database`
  でその applications ディレクトリの MIME キャッシュを更新する。
  既定 MIME は変更せず、選択は GNOME 設定またはユーザーの明示的な xdg-mime 操作に委ねる。
- Linux の起動時常駐は設定画面から
  `$XDG_CONFIG_HOME/autostart/brows.desktop` に実行ファイルの絶対パスと `--resident` を登録する。
  既定の配置先は `~/.config/autostart/brows.desktop`。
- バイナリを移動した場合は登録先のパスも更新する。
  登録解除は代わりの既定ブラウザを選ばず、起動時常駐とは別の操作である。

## プロジェクト構成

```text
src/
├── main.rs              起動引数の振り分け
├── browser.rs           共通モデル、プロファイル JSON、バックグラウンド再検出
├── browser/
│   ├── linux.rs         GIO Desktop Entry 検出・起動、通常パッケージのデータルート
│   └── windows.rs       レジストリ検出、Windows のデータルート
├── config.rs            TOML 設定、プロセス間ロック、一時ファイル置換
├── ipc.rs               HTTP/HTTPS 検証と OS 別 IPC の境界
├── ipc/
│   ├── linux.rs         Unix domain socket、本人の資格確認、常駐ロック
│   └── windows.rs       ユーザー SID と DACL を使う名前付きパイプ
├── url_queue.rs         URL を FIFO で保持する共通状態
├── registry.rs          OS 別登録 API の境界
├── registry/
│   ├── linux.rs         Desktop Entry と autostart
│   └── windows.rs       レジストリ、HKCU Run、UAC
├── icon.rs              OS 別アイコン API の境界
├── icon_linux.rs        GIO Desktop Entry の PNG と hicolor / pixmaps
├── icon_windows.rs      Windows Shell / GDI
├── lang.rs              日本語・英語と OS 別文言
├── updater.rs           OS 別更新 API の境界（Linux は無効）
├── updater_windows.rs   GitHub Releases の exe 更新・再起動
├── util.rs              子プロセス、設定ファイルの関連アプリ起動、権限判定
└── ui/
    ├── mod.rs           共通セットアップ、OS 別フォント
    ├── picker.rs        常駐ピッカー、IPC 受信とキュー統合
    ├── settings.rs      登録、起動時常駐、URL ルール、Windows 更新
    ├── window.rs        OS 別ウィンドウ API の境界
    ├── x11.rs           x11rb による表示・非表示・位置・フォーカス要求
    ├── win32.rs         Win32 による表示・非表示・位置・フォーカス
    ├── tray.rs          OS 別トレイ API の境界（Linux は無効）
    └── tray_windows.rs  Windows トレイと別スレッドのメッセージループ
```

共通モデルと UI を維持し、OS 固有の import / link / resource 処理は cfg の境界に置く。
Linux は eframe で X11 を指定し、winit のイベントループも X11 に明示固定する。
glutin-winit が Wayland を推移的に有効にするため、feature 指定だけに依存しない。
新しい常駐サービスや別のイベントループを増やす前に既存の仕組みと標準機能を検討する。

## 常駐と URL の処理

`--resident` は冪等で、同じユーザーの常駐があれば即終了する。
引数なし起動は設定画面を開き、通常権限なら別プロセスの `--resident` も確保する。
管理者 / root の設定画面から自動で昇格した常駐を作らない。

- Windows: `\\.\pipe\brows.<SID>` の最初のインスタンス作成で判定し、
  DACL を本人と SYSTEM に限定する。
- Linux: 本人所有で mode 0700 の XDG_RUNTIME_DIR に `brows/` を作る。
  lifetime lock と `resident.sock` を使い、SO_PEERCRED で同じユーザーのクライアントに限定する。
  起動競合では接続準備を待つが、配送後の ACK 不明時に URL を再送して重複させない。
- 受信 URL は FIFO に保存し、表示中の URL を後続リンクで上書きしない。
  選択・キャンセル後に次へ進み、ブラウザ起動失敗では URL を維持してエラーを表示する。
- 非表示ウィンドウの再表示は OS API で行う。
  Linux は winit の借用 Display を IPC スレッドから触らず、別の x11rb 接続を使う。
  GNOME のフォーカス要求受理は実機で確認する。
- Windows のトレイは別スレッドの Shell_NotifyIconW で表示し、終了前に cleanup する。
  Explorer 再起動時は TaskbarCreated で再登録する。
  Windows 自動更新の再起動前は BROWS-EXIT で旧常駐を終了する。
- Linux はトレイを作らず、設定への導線は引数なし起動、常駐終了は `--quit` を使う。

## 設定ファイル

Windows は `%APPDATA%\brows\config.toml`、Linux は
`$XDG_CONFIG_HOME/brows/config.toml`（既定 `~/.config/brows/config.toml`）に保存する。

**設定への書き込みは必ず `Config::update` 経由で行うこと。**
最新の設定を読み、変更するフィールドだけを適用し、プロセス間ロックのもとで
一時ファイルを置換する。in-memory の Config 全体を保存すると、設定画面と
常駐側のバックグラウンド書き込みを巻き戻す。破損した設定を既定値で上書きしない。

URL ルールは先頭からの、大文字小文字を区別する部分一致。
Linux のブラウザ・並び順には Desktop Entry ID を使い、プロファイルには
`<desktop-id>#<profile-directory>` を使う。旧設定の実行パスによる並び順、
ブラウザ表示名による指定も読み込み時に扱う。

```toml
# Linux の例。ID は検出した Desktop Entry に合わせる。
default_browser = "vivaldi-stable.desktop#Default"
browser_order = ["vivaldi-stable.desktop", "google-chrome.desktop"]

[[rules]]
pattern = "github.com"
browser = "vivaldi-stable.desktop#Default"
```

## Git ワークフロー

**必ず feature ブランチを切ってから作業する。**
既存 checkout と未コミット変更を確認し、必要なら独立 worktree で作業する。

```sh
git status --short --branch
git checkout -b feature/xxx
# 実装と検証後、承認された範囲でコミットする
git add <files>
git commit -m "変更が必要な理由"
```

`main` への直接 push はしない。push / PR / merge / 配布物公開は、
その操作についてのユーザーの承認に従う。
コードには How、テストには What、コミットログには Why、コードコメントには Why not を記す。

## OS 別の注意点

- Windows は windows_subsystem を指定する。Linux は通常のコンソール出力を使う。
- Chromium 系以外のブラウザはプロファイル展開非対応。
- Windows の日本語フォントは YuGothM.ttc → meiryo.ttc → msgothic.ttc。
  Linux はシステムの NotoSansCJK-Regular.ttc を使う。
  ファイルが存在しても日本語描画の確認を省略しない。
- Linux の Desktop Entry Exec はシェルコマンドではない。
  引数の引用・エスケープと field code を標準の GIO / KeyFile に従って扱う。
  ユーザーの実ブラウザや実プロファイルを検証用 fixture として使わない。
