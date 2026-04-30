# brows

Windows 11 向けブラウザ選択ツール。リンクを開くたびに、インストール済みブラウザの一覧からどれで開くか選べます。

<img width="448" height="415" alt="image" src="https://github.com/user-attachments/assets/b6dc38c2-1266-4bce-9740-96b717296c30" />

<img width="419" height="343" alt="image" src="https://github.com/user-attachments/assets/9be684ce-90f1-45d7-85f9-fb211928b176" />

## 特徴

- インストール済みブラウザを自動検出（Chrome / Edge / Vivaldi / Brave など）
- Chrome / Edge など Chromium 系ブラウザはプロファイルごとに選択可能
- ブラウザの表示順をドラッグで変更・保存
- URL パターンに応じて自動でブラウザを選択するルールを GUI で設定
- 起動時にバックグラウンドでアップデートを確認、設定画面から更新可能

## インストール

1. [Releases](https://github.com/gentianster/brows/releases/latest) から `brows.exe` をダウンロード
2. 任意のフォルダに配置
3. `brows.exe` をダブルクリックして設定画面を開く
4. **「既定ブラウザとして登録」** をクリック（UAC プロンプトが表示されます）
5. Windows 設定 → アプリ → 既定のアプリ → **brows** を既定のブラウザに設定

## 使い方

登録後は、任意のアプリからリンクを開くと自動的にブラウザ選択ダイアログが表示されます。

### ブラウザの順序変更

ダイアログ上でブラウザ行をドラッグして並び順を変更できます。順序は自動保存されます。

### URL ルール設定

設定画面の「URL ルール」セクションから、URL パターンに応じたブラウザの自動選択を設定できます。パターンにマッチした URL はブラウザ選択ダイアログを表示せず、直接指定のブラウザで開きます。

Chrome などのプロファイルも指定可能です（プロファイルの表示名で選択）。

設定は `%APPDATA%\brows\config.toml` に保存されます。

## ビルド

```bash
cargo build --release
```

`target/release/brows.exe` が生成されます。Rust ツールチェインのみ必要で、外部ツール不要です。

## ライセンス

MIT License

## 要件

- Windows 11
- 既定ブラウザ登録には管理者権限が必要
