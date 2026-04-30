use std::sync::{Arc, Mutex};

const REPO: &str = "gentianster/brows";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Checking,
    UpToDate,
    Available(String), // 新バージョンのタグ名 (e.g. "v0.2.0")
    Downloading,
    ReadyToRestart,
    Error(String),
}

#[derive(Clone)]
pub struct Updater {
    pub state: Arc<Mutex<UpdateState>>,
}

impl Updater {
    pub fn start() -> Self {
        let state = Arc::new(Mutex::new(UpdateState::Checking));
        let state_clone = state.clone();

        std::thread::spawn(move || {
            let result = match fetch_latest_tag() {
                Some(tag) if is_newer(&tag) => UpdateState::Available(tag),
                Some(_) => UpdateState::UpToDate,
                None => UpdateState::UpToDate, // API 失敗時は黙って無視
            };
            *state_clone.lock().unwrap() = result;
        });

        Self { state }
    }

    pub fn download_and_restart(&self) {
        let tag = match &*self.state.lock().unwrap() {
            UpdateState::Available(t) => t.clone(),
            _ => return,
        };
        *self.state.lock().unwrap() = UpdateState::Downloading;
        let state = self.state.clone();

        std::thread::spawn(move || {
            match do_download(&tag) {
                Ok(_) => *state.lock().unwrap() = UpdateState::ReadyToRestart,
                Err(e) => *state.lock().unwrap() = UpdateState::Error(e),
            }
        });
    }

    pub fn restart() {
        // バッチファイルで: 現 exe を .old にリネームし新 exe を配置して再起動
        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp_exe = std::env::temp_dir().join("brows_update.exe");
        let old_exe = current_exe.with_extension("old.exe");
        let bat = std::env::temp_dir().join("brows_update.bat");

        let script = format!(
            "@echo off\r\ntimeout /t 1 /nobreak >nul\r\nmove /y \"{old}\" \"{backup}\"\r\nmove /y \"{new}\" \"{cur}\"\r\nstart \"\" \"{cur}\"\r\ndel \"%~f0\"",
            cur = current_exe.display(),
            backup = old_exe.display(),
            new = tmp_exe.display(),
            old = current_exe.display(),
        );
        let _ = std::fs::write(&bat, script);
        let _ = std::process::Command::new("cmd").args(["/c", &bat.to_string_lossy()]).spawn();
        std::process::exit(0);
    }
}

/// GitHub Releases API から最新タグを取得
fn fetch_latest_tag() -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let output = std::process::Command::new("curl")
        .args(["-s", "--connect-timeout", "5", "-A", "brows-updater", &url])
        .output()
        .ok()?;
    let body = String::from_utf8(output.stdout).ok()?;
    json_str(&body, "tag_name")
}

/// 新バージョン exe を %TEMP%\brows_update.exe にダウンロード
fn do_download(tag: &str) -> Result<(), String> {
    let url = format!(
        "https://github.com/{}/releases/download/{}/brows.exe",
        REPO, tag
    );
    let dest = std::env::temp_dir().join("brows_update.exe");
    let status = std::process::Command::new("curl")
        .args(["-sL", "--connect-timeout", "30", "-o", &dest.to_string_lossy(), &url])
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() { Ok(()) } else { Err("ダウンロード失敗".into()) }
}

/// 文字列バージョン比較（"v0.2.0" > "0.1.0" → true）
fn is_newer(tag: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v').split('.').filter_map(|p| p.parse().ok()).collect()
    };
    parse(tag) > parse(CURRENT)
}

fn json_str(text: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\":", key);
    let pos = text.find(&search)?;
    let after = text[pos + search.len()..].trim_start();
    if after.starts_with('"') {
        let end = after[1..].find('"')?;
        Some(after[1..end + 1].to_string())
    } else {
        None
    }
}
