use anyhow::Result;
use serde::{Deserialize, Serialize};
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Browser {
    pub name: String,
    pub exe_path: String,
    pub profile_dir: Option<String>, // --profile-directory 引数
}

impl Browser {
    pub fn launch(&self, url: &str) -> Result<()> {
        let mut cmd = std::process::Command::new(&self.exe_path);
        if let Some(dir) = &self.profile_dir {
            cmd.arg(format!("--profile-directory={}", dir));
        }
        cmd.arg(url).spawn()?;
        Ok(())
    }
}

/// `"C:\path\browser.exe" "%1"` 形式から exe パスだけを取り出す
fn extract_exe(cmd: &str) -> String {
    let cmd = cmd.trim();
    if cmd.starts_with('"') {
        cmd[1..].splitn(2, '"').next().unwrap_or("").to_string()
    } else if let Some(pos) = cmd.to_lowercase().find(".exe") {
        cmd[..pos + 4].to_string()
    } else {
        cmd.splitn(2, ' ').next().unwrap_or(cmd).to_string()
    }
}

/// キーの既定値 → Capabilities\ApplicationName → キー名、の順で表示名を解決する
fn display_name(browser_key: &RegKey, fallback: &str) -> String {
    browser_key
        .get_value::<String, _>("")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            browser_key
                .open_subkey("Capabilities")
                .ok()
                .and_then(|cap| cap.get_value::<String, _>("ApplicationName").ok())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Chromium 系ブラウザの User Data ディレクトリを返す
fn chromium_user_data_dir(exe_path: &str) -> Option<std::path::PathBuf> {
    let exe_name = std::path::Path::new(exe_path)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let base = std::path::PathBuf::from(local);
    let dir = match exe_name.as_str() {
        "chrome.exe"   => base.join("Google").join("Chrome").join("User Data"),
        "msedge.exe"   => base.join("Microsoft").join("Edge").join("User Data"),
        "vivaldi.exe"  => base.join("Vivaldi").join("User Data"),
        "brave.exe"    => base.join("BraveSoftware").join("Brave-Browser").join("User Data"),
        _ => return None,
    };
    dir.exists().then_some(dir)
}

/// User Data の Local State から (表示名, ディレクトリ名) の一覧を返す
fn read_profiles(user_data_dir: &std::path::Path) -> Vec<(String, String)> {
    try_read_profiles(user_data_dir).unwrap_or_default()
}

fn try_read_profiles(user_data_dir: &std::path::Path) -> Option<Vec<(String, String)>> {
    let local_state = std::fs::read_to_string(user_data_dir.join("Local State")).ok()?;

    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(user_data_dir).ok()?.flatten() {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if dir_name != "Default" && !dir_name.starts_with("Profile ") {
            continue;
        }
        if !entry.path().join("Preferences").exists() {
            continue;
        }
        let display = profile_name_from_local_state(&local_state, &dir_name)
            .unwrap_or_else(|| dir_name.clone());
        profiles.push((display, dir_name));
    }

    if profiles.is_empty() {
        return None;
    }

    profiles.sort_by(|a, b| match (a.1.as_str(), b.1.as_str()) {
        ("Default", _) => std::cmp::Ordering::Less,
        (_, "Default") => std::cmp::Ordering::Greater,
        _ => a.1.cmp(&b.1),
    });
    Some(profiles)
}

/// Local State の info_cache から指定プロファイルの "name" を取り出す
fn profile_name_from_local_state(local_state: &str, dir_name: &str) -> Option<String> {
    let cache_pos = local_state.find("\"info_cache\":")?;
    let cache_start = local_state[cache_pos..].find('{')? + cache_pos + 1;
    let cache_body = &local_state[cache_start..];

    let key = format!("\"{}\":", dir_name);
    let key_pos = cache_body.find(&key)?;
    let after_key = &cache_body[key_pos + key.len()..];
    let obj_start = after_key.find('{')? + 1;
    let obj_body = &after_key[obj_start..obj_start.saturating_add(2048)];

    json_str(obj_body, "name")
}

/// JSON テキストから `"key": "value"` の value を返す（簡易・ネストなし）
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

/// インストール済みブラウザをレジストリから検出する（設定画面用・プロファイル展開なし）
pub fn detect() -> Result<Vec<Browser>> {
    detect_base()
}

/// ピッカー用グループ。browsers が 1 件なら直接起動、複数ならプロファイル選択
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserGroup {
    pub name: String,
    pub exe_path: String,
    pub browsers: Vec<Browser>,
}

/// ピッカー用。Chromium 系は複数プロファイルがあれば BrowserGroup にまとめる
pub fn detect_grouped() -> Result<Vec<BrowserGroup>> {
    let base = detect_base()?;
    let mut groups = Vec::new();

    for b in base {
        if let Some(user_data_dir) = chromium_user_data_dir(&b.exe_path) {
            let profiles = read_profiles(&user_data_dir);
            if profiles.len() > 1 {
                let browsers = profiles
                    .into_iter()
                    .map(|(profile_name, profile_dir)| Browser {
                        name: profile_name,
                        exe_path: b.exe_path.clone(),
                        profile_dir: Some(profile_dir),
                    })
                    .collect();
                groups.push(BrowserGroup { name: b.name, exe_path: b.exe_path, browsers });
                continue;
            }
        }
        groups.push(BrowserGroup {
            name: b.name.clone(),
            exe_path: b.exe_path.clone(),
            browsers: vec![b],
        });
    }

    Ok(groups)
}

fn detect_base() -> Result<Vec<Browser>> {
    let mut browsers = Vec::new();

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let clients_key = hklm.open_subkey("SOFTWARE\\Clients\\StartMenuInternet")?;

    for key_name in clients_key.enum_keys().flatten() {
        if let Ok(browser_key) = clients_key.open_subkey(&key_name) {
            if let Ok(cmd_key) = browser_key.open_subkey("shell\\open\\command") {
                let exe_path: String = cmd_key.get_value("").unwrap_or_default();
                let exe_path = extract_exe(&exe_path);
                if !exe_path.is_empty() {
                    let name = display_name(&browser_key, &key_name);
                    browsers.push(Browser { name, exe_path, profile_dir: None });
                }
            }
        }
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(clients_key) = hkcu.open_subkey("SOFTWARE\\Clients\\StartMenuInternet") {
        for key_name in clients_key.enum_keys().flatten() {
            if let Ok(browser_key) = clients_key.open_subkey(&key_name) {
                if let Ok(cmd_key) = browser_key.open_subkey("shell\\open\\command") {
                    let exe_path: String = cmd_key.get_value("").unwrap_or_default();
                    let exe_path = extract_exe(&exe_path);
                    if !exe_path.is_empty() {
                        let name = display_name(&browser_key, &key_name);
                        if !browsers.iter().any(|b| b.exe_path == exe_path) {
                            browsers.push(Browser { name, exe_path, profile_dir: None });
                        }
                    }
                }
            }
        }
    }

    let self_exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));

    browsers.retain(|b| {
        let path = std::path::Path::new(&b.exe_path);
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
        !b.exe_path.to_lowercase().ends_with("iexplore.exe")
            && self_exe_name.as_deref().map_or(true, |s| file_name.as_deref() != Some(s))
    });

    Ok(browsers)
}
