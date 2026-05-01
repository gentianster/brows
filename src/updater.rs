use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const REPO: &str = "gentianster/brows";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL_SECS: u64 = 86400; // 1日

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    UpToDate,
    Available(String),
    Downloading,
    ReadyToRestart,
    Error(String),
}

#[derive(Clone)]
pub struct Updater {
    pub state: Arc<Mutex<UpdateState>>,
}

impl Updater {
    /// config から既知の更新情報を読んで初期化。チェック期限が来ていれば
    /// バックグラウンドで API を叩いて state と config を両方更新する。
    pub fn from_config(config: &crate::config::Config) -> Self {
        let initial = match &config.update_available {
            Some(tag) if is_newer(tag) => UpdateState::Available(tag.clone()),
            _ => UpdateState::UpToDate,
        };
        let state = Arc::new(Mutex::new(initial));

        if is_due(config.last_update_check) {
            let state_clone = state.clone();
            std::thread::spawn(move || {
                let now = unix_now();
                let mut cfg = crate::config::Config::load().unwrap_or_default();
                cfg.last_update_check = Some(now);
                match fetch_latest_tag() {
                    Some(tag) if is_newer(&tag) => {
                        cfg.update_available = Some(tag.clone());
                        *state_clone.lock().unwrap() = UpdateState::Available(tag);
                    }
                    Some(_) => {
                        cfg.update_available = None;
                        *state_clone.lock().unwrap() = UpdateState::UpToDate;
                    }
                    None => {}
                }
                let _ = cfg.save();
            });
        }

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
        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let tmp_exe = std::env::temp_dir().join("brows_update.exe");
        let old_exe = current_exe.with_extension("old.exe");
        let bat = std::env::temp_dir().join("brows_update.bat");

        let script = format!(
            concat!(
                "@echo off\r\n",
                "timeout /t 2 /nobreak >nul\r\n",
                // 現在の exe をバックアップに移動。失敗したら新ファイルを消して終了
                "move /y \"{old}\" \"{backup}\"\r\n",
                "if errorlevel 1 (\r\n",
                "  del \"{new}\" 2>nul\r\n",
                "  del \"%~f0\"\r\n",
                "  exit /b 1\r\n",
                ")\r\n",
                // 新しい exe を配置。失敗したらバックアップを元に戻して終了
                "move /y \"{new}\" \"{cur}\"\r\n",
                "if errorlevel 1 (\r\n",
                "  move /y \"{backup}\" \"{cur}\" 2>nul\r\n",
                "  del \"{new}\" 2>nul\r\n",
                "  del \"%~f0\"\r\n",
                "  exit /b 1\r\n",
                ")\r\n",
                "start \"\" \"{cur}\"\r\n",
                "del \"%~f0\""
            ),
            cur = current_exe.display(),
            backup = old_exe.display(),
            new = tmp_exe.display(),
            old = current_exe.display(),
        );
        let _ = std::fs::write(&bat, script);
        let _ = std::process::Command::new("cmd")
            .args(["/c", &bat.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        std::process::exit(0);
    }
}

/// 起動時に呼ぶ。CHECK_INTERVAL_SECS 経過していればバックグラウンドでチェックして config に保存する
pub fn check_if_due() {
    let config = match crate::config::Config::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    if !is_due(config.last_update_check) {
        return;
    }
    std::thread::spawn(move || {
        let now = unix_now();
        let mut cfg = crate::config::Config::load().unwrap_or_default();
        cfg.last_update_check = Some(now);
        match fetch_latest_tag() {
            Some(tag) if is_newer(&tag) => cfg.update_available = Some(tag),
            Some(_) => cfg.update_available = None,
            None => {} // API 失敗時は前回の結果を維持
        }
        let _ = cfg.save();
    });
}

fn is_due(last: Option<u64>) -> bool {
    match last {
        None => true,
        Some(t) => unix_now().saturating_sub(t) >= CHECK_INTERVAL_SECS,
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fetch_latest_tag() -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let output = std::process::Command::new("curl")
        .args(["-s", "--connect-timeout", "5", "-A", "brows-updater", &url])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let body = String::from_utf8(output.stdout).ok()?;
    json_str(&body, "tag_name")
}

fn do_download(tag: &str) -> Result<(), String> {
    let url = format!(
        "https://github.com/{}/releases/download/{}/brows.exe",
        REPO, tag
    );
    let dest = std::env::temp_dir().join("brows_update.exe");
    let status = std::process::Command::new("curl")
        .args(["-sL", "--connect-timeout", "30", "-o", &dest.to_string_lossy(), &url])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() { Ok(()) } else { Err("ダウンロード失敗".into()) }
}

pub fn is_newer(tag: &str) -> bool {
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
