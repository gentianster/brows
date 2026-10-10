# Linux 対応と検証記録

対象は Ubuntu 26.04 x86_64 の Wayland セッション上で動く XWayland アプリ。
常駐プロセスと同じピッカーを保持し、選択・キャンセル後に非表示、次の URL で再表示する。
既存の Windows 11 実装は OS 境界に分離して維持する。

## 実装の範囲

- Linux の eframe / glow と winit は X11 を明示選択する。
  glutin-winit の推移的 Wayland feature によってバックエンドが変わることを防ぐ。
- GIO の Desktop Entry で HTTP/HTTPS の URI ハンドラーを検出・起動する。
  Desktop Entry ID、元の固定引数、field code を維持し、brows 自身と非表示 Entry を除く。
  候補は WebBrowser カテゴリーに限定しないため、リンクを扱うアプリも表示対象。
- 通常パッケージの Vivaldi / Chrome / Chromium は Local State のプロファイル名と
  ディレクトリの存在を使う。履歴・Cookie・資格情報は読まない。
  複数プロファイルの表示名と保存用 ID を分離し、元の固定引数と `--profile-directory` を維持する。
- Linux の Unix domain socket は本人所有の private XDG_RUNTIME_DIR、常駐ロック、
  socket 権限と peer UID を使う。初回の起動競合を待ち、受信後の ACK 失敗は再送せず報告する。
- URL は FIFO に保持し、表示中の URL を上書きしない。選択・キャンセル後に次へ進み、
  ブラウザ起動エラーでは URL とピッカーを維持する。
- 設定は `Config::update`、プロセス間ロック、一時ファイル置換で保存する。
  並び順、キャッシュ、ルールの同時変更が互いに巻き戻らないようにする。
- Linux の登録はユーザー Desktop Entry と applications の MIME cache を使う。
  既定ブラウザの選択はユーザーが行う。起動時常駐は設定画面からの明示操作。
- Linux の設定画面は引数なし起動、常駐終了は `brows --quit`。
  日本語フォントはシステムの Noto CJK を使う。
- ブラウザ画像アイコンは Desktop Entry の絶対 PNG、hicolor、XDG icons、旧 `.icons`、
  pixmaps に対応する。PNG の読み込み量・寸法・デコード量を制限し、FIFO では停止しない。
- Linux の一覧は候補や展開プロファイルが増えてもスクロール領域内に収め、下部操作を残す。
  Windows の既存行高・配色・Shell アイコン取得は維持する。

コードには How、テストには What、コミットログには Why、コードコメントには Why not を記す。

## 検証環境とビルド

ベースは `main` / `5efe094` (v1.0.15)、実装ブランチは `feature/linux-xwayland`。
元の checkout を保持し、独立 worktree で作業した。AGENTS.md と関連 .agents/skills はなく、
CLAUDE.md の feature ブランチと `Config::update` の規約を適用した。

Ubuntu 26.04.1 LTS / x86_64 / Wayland + XWayland、Rust/Cargo 1.93.1、GIO 2.88.0 で確認した。
ビルド用パッケージは README.md / README.ja.md を参照する。
X11/XKB は実行時共有ライブラリを動的に使うため、この構成では X11/XKB/Wayland/GL の
開発用パッケージを追加せずビルドできた。

```sh
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline
cargo build --release --locked --offline
```

依存を取得した後の最終確認は `--locked --offline` で行った。

## 確認結果（2026-10-10）

| 確認 | 結果と限界 |
| --- | --- |
| Rust テスト | 初回 33 本、追加修正後 40 本成功。設定更新の子プロセス試験を含む |
| fmt / clippy / diff check | 成功。clippy は all-targets と -D warnings |
| Linux release build | 成功。x86_64 ELF、初回ビルドでは実行時共有ライブラリの欠落なし |
| Windows native build | 未実行。このホストに Windows 用標準ライブラリ・ツールチェインがない |
| 合成 Desktop Entry / GIO 起動 | 登録・autostart の検証、固定引数・URI・プロファイル・特殊文字パスが一致 |
| IPC / 設定競合 | 単一常駐、socket 回復、並行 URL、ACK 失敗、FIFO 状態、別プロセス更新を確認 |
| XWayland ライフサイクル | 表示修正前のビルドで 14/14 成功。close 後に同じ PID/XID を保持し、次の URL で再表示・フォーカス |
| 初回 URL ルール | 合成ランチャーへ固定引数と Default を渡し、常駐を保持して後続 URL で再表示 |
| 日本語 | 合成ピッカー・設定画面の画像で確認 |
| 人による合成プロファイル選択 | 表示修正前に個人・仕事用を選択。捕捉ログの Default / Profile 1 と URL・固定引数が一致 |
| 表示修正後の画面 | 7 件の候補、PNG、非表示別名除外、展開状態で下部操作が見えることを確認 |
| 独立レビュー | OS 境界と修正を再確認し、新たな重大な回帰なし。Windows 実機ビルドの代わりにはしない |

40 本の試験は合成 Desktop Entry、JSON プロファイル metadata、捕捉ランチャー、
一時ディレクトリを使う。通常 sandbox では Unix socket 作成が禁止されるため、
全テストは審査付きで合成データだけを使った。承認審査の拒否はなかった。

初回 GUI 試験のバイナリ SHA-256 は
`68a7fa1dc8b2cfcce615256233de21a3ef493a3223b784902882fec394446a3f`。
表示修正後の release バイナリ SHA-256 は
`8044427d6727fa1a31f1fb42f920b5fad5a87792ceb7b7599a02a878eec5ee9e`。
前者の GUI 結果を後者の全操作の成功として扱わない。

## 画面試験の範囲

[初回 GUI 記録](verification/linux-xwayland-2026-10-10/README.md) と
[合成環境の手動手順](../tests/manual/README.md) を参照する。
HOME と XDG config/data/runtime/cache、Chromium の指定を隔離し、仮ブラウザだけを使った。
画像取得は試験開始した PID の client window に限定した。
公開記録から一時ディレクトリの絶対パス、PID、XID を除き、観測結果を保持した。
一時パスが画素に残る設定画像と、実ユーザーが提供した画像は公開記録に含めていない。

初回の自動選択試験では XTest が成功値を返してもポインターが目標に到達しなかった。
目標座標に対して実座標が異なることを確認し、ボタン入力前に停止する判定を追加した。
追加の入力調整は行わず、標準 WM_DELETE_WINDOW で共通 dismiss 経路を試験した。
別の初期フォーカス失敗も保存しており、GNOME が全てのフォーカス要求を受理するとは保証しない。

表示修正後は模擬入力を再開せず、通常一覧を release バイナリで撮影した。
展開状態はソースの一時コピーから同じ PickerApp に初期状態を与える専用ビルドで撮影した。
Chrome / Vivaldi / Firefox / ChatGPT の公開 PNG を隔離環境に複製し、
テーマ名・絶対パス・pixmaps 経由の表示を確認した。7 件の候補と非表示別名の除外も確認した。
その画像はローカル確認だけに使用した。撮影は XGetImage のみで、libXtst をロードせず、
XTest / CUA / RemoteDesktop / ScreenCast は使っていない。本物のブラウザは起動していない。

## 残る確認と後続範囲

- Windows ホストでの native build と実機回帰確認。
- 表示修正後のクリックによる展開・選択、スクロール入力、選択時 FIFO、キャンセルボタン。
  人が確認した 2 件のプロファイル選択だけで全体の実 UI 成功とは扱わない。
- ネイティブ Wayland、Snap/Flatpak のプロファイル選択、自動更新、トレイ。
- SVG アイコンと使用中テーマの index.theme 継承、Linux 配布物の公開。
- 候補を WebBrowser カテゴリーに限定するかは設計判断。現在は表示可能な HTTP/HTTPS ハンドラー。

GitHub の既存 workflow は v* タグによる Windows リリース専用。
PR のビルド・テスト CI は構成されておらず、リリース workflow はこの検証では起動しない。

主な根拠は [依存の OS 境界](../Cargo.toml)、[ブラウザ処理と試験](../src/browser/linux.rs)、
[IPC 回帰試験](../src/ipc/linux.rs)、[FIFO](../src/url_queue.rs)、
[設定保存と別プロセス試験](../src/config.rs)、[登録の隔離試験](../src/registry/linux.rs)、
[PNG と回帰試験](../src/icon_linux.rs)、[X11 選択](../src/ui/mod.rs)、
[保持ウィンドウ](../src/ui/x11.rs)、[ピッカー統合](../src/ui/picker.rs) にある。
