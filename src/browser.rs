use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Clone)]
pub struct Browser {
    pub name: String,
    pub exe_path: String,
}

impl Browser {
    pub fn launch(&self, url: &str) -> Result<()> {
        std::process::Command::new(&self.exe_path)
            .arg(url)
            .spawn()?;
        Ok(())
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

/// インストール済みブラウザをレジストリから検出する
pub fn detect() -> Result<Vec<Browser>> {
    let mut browsers = Vec::new();

    // HKLM\SOFTWARE\Clients\StartMenuInternet 以下を探す
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let clients_key = hklm
        .open_subkey("SOFTWARE\\Clients\\StartMenuInternet")?;

    for key_name in clients_key.enum_keys().flatten() {
        if let Ok(browser_key) = clients_key.open_subkey(&key_name) {
            if let Ok(cmd_key) = browser_key.open_subkey("shell\\open\\command") {
                let exe_path: String = cmd_key.get_value("").unwrap_or_default();
                let exe_path = exe_path.trim_matches('"').to_string();
                if !exe_path.is_empty() {
                    let name = display_name(&browser_key, &key_name);
                    browsers.push(Browser { name, exe_path });
                }
            }
        }
    }

    // HKCU も確認（ユーザーインストールのブラウザ対応）
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(clients_key) = hkcu.open_subkey("SOFTWARE\\Clients\\StartMenuInternet") {
        for key_name in clients_key.enum_keys().flatten() {
            if let Ok(browser_key) = clients_key.open_subkey(&key_name) {
                if let Ok(cmd_key) = browser_key.open_subkey("shell\\open\\command") {
                    let exe_path: String = cmd_key.get_value("").unwrap_or_default();
                    let exe_path = exe_path.trim_matches('"').to_string();
                    if !exe_path.is_empty() {
                        let name = display_name(&browser_key, &key_name);
                        // 重複チェック（exe パスで判定）
                        if !browsers.iter().any(|b| b.exe_path == exe_path) {
                            browsers.push(Browser { name, exe_path });
                        }
                    }
                }
            }
        }
    }

    browsers.retain(|b| {
        !b.exe_path.to_lowercase().ends_with("iexplore.exe")
    });

    Ok(browsers)
}