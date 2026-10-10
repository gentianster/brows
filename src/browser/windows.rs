use super::Browser;
use anyhow::Result;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

fn extract_exe(cmd: &str) -> String {
    let cmd = cmd.trim();
    if let Some(quoted) = cmd.strip_prefix('"') {
        quoted.split('"').next().unwrap_or("").to_string()
    } else if let Some(pos) = cmd.to_lowercase().find(".exe") {
        cmd[..pos + 4].to_string()
    } else {
        cmd.split(' ').next().unwrap_or(cmd).to_string()
    }
}

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

pub(super) fn user_data_dir(browser: &Browser) -> Option<PathBuf> {
    let exe_name = std::path::Path::new(&browser.exe_path)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?);
    let dir = match exe_name.as_str() {
        "chrome.exe" => base.join("Google").join("Chrome").join("User Data"),
        "msedge.exe" => base.join("Microsoft").join("Edge").join("User Data"),
        "vivaldi.exe" => base.join("Vivaldi").join("User Data"),
        "brave.exe" => base
            .join("BraveSoftware")
            .join("Brave-Browser")
            .join("User Data"),
        _ => return None,
    };
    dir.is_dir().then_some(dir)
}

fn collect_clients(root: &RegKey, browsers: &mut Vec<Browser>) {
    let Ok(clients_key) = root.open_subkey("SOFTWARE\\Clients\\StartMenuInternet") else {
        return;
    };
    for key_name in clients_key.enum_keys().flatten() {
        if let Ok(browser_key) = clients_key.open_subkey(&key_name) {
            if let Ok(cmd_key) = browser_key.open_subkey("shell\\open\\command") {
                let exe_path: String = cmd_key.get_value("").unwrap_or_default();
                let exe_path = extract_exe(&exe_path);
                if !exe_path.is_empty() && !browsers.iter().any(|b| b.exe_path == exe_path) {
                    browsers.push(Browser {
                        name: display_name(&browser_key, &key_name),
                        exe_path,
                        profile_dir: None,
                        desktop_id: None,
                        desktop_file: None,
                        desktop_exec: None,
                    });
                }
            }
        }
    }
}

pub(super) fn detect() -> Result<Vec<Browser>> {
    let mut browsers = Vec::new();
    collect_clients(&RegKey::predef(HKEY_LOCAL_MACHINE), &mut browsers);
    collect_clients(&RegKey::predef(HKEY_CURRENT_USER), &mut browsers);
    let self_exe_name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    browsers.retain(|b| {
        let file_name = std::path::Path::new(&b.exe_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase());
        !b.exe_path.to_lowercase().ends_with("iexplore.exe")
            && self_exe_name
                .as_deref()
                .is_none_or(|s| file_name.as_deref() != Some(s))
    });
    Ok(browsers)
}
