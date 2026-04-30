use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "brows";

/// Windowsにブラウザとして登録する
/// 管理者権限が必要
pub fn register() -> Result<()> {
    let exe_path = std::env::current_exe()?
        .to_string_lossy()
        .to_string();
    let open_cmd = format!("\"{}\" \"%1\"", exe_path);

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 1. CapabilitiesキーにStartMenuInternetとして登録
    let (cap_key, _) = hklm.create_subkey(format!(
        "SOFTWARE\\Clients\\StartMenuInternet\\{}\\Capabilities",
        APP_NAME
    ))?;
    cap_key.set_value("ApplicationName", &APP_NAME)?;
    cap_key.set_value("ApplicationDescription", &"Browser picker for Windows")?;

    // URLの関連付け
    let (url_assoc, _) = cap_key.create_subkey("URLAssociations")?;
    url_assoc.set_value("http", &format!("{}URL", APP_NAME))?;
    url_assoc.set_value("https", &format!("{}URL", APP_NAME))?;

    // 2. shell\open\command に実行パスを登録
    let (cmd_key, _) = hklm.create_subkey(format!(
        "SOFTWARE\\Clients\\StartMenuInternet\\{}\\shell\\open\\command",
        APP_NAME
    ))?;
    cmd_key.set_value("", &open_cmd)?;

    // 3. URLハンドラの登録 (HKCR)
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    for protocol in &["http", "https"] {
        let key_name = format!("{}URL\\shell\\open\\command", APP_NAME);
        let (proto_key, _) = hkcr.create_subkey(&key_name)?;
        proto_key.set_value("", &open_cmd)?;

        // プロトコルとの関連付け
        let (cap_key, _) = hkcr.create_subkey(format!("{}URL", APP_NAME))?;
        cap_key.set_value("", &format!("URL:{} Protocol", protocol))?;
        cap_key.set_value("URL Protocol", &"")?;
    }

    // 4. RegisteredApplications に追加
    let (reg_apps, _) = hklm.create_subkey("SOFTWARE\\RegisteredApplications")?;
    reg_apps.set_value(
        APP_NAME,
        &format!("SOFTWARE\\Clients\\StartMenuInternet\\{}\\Capabilities", APP_NAME),
    )?;

    Ok(())
}

/// 登録を解除する
pub fn unregister() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // RegisteredApplications から削除
    if let Ok(reg_apps) = hklm.open_subkey_with_flags(
        "SOFTWARE\\RegisteredApplications",
        KEY_WRITE,
    ) {
        let _ = reg_apps.delete_value(APP_NAME);
    }

    // StartMenuInternet から削除
    let _ = hklm.delete_subkey_all(format!(
        "SOFTWARE\\Clients\\StartMenuInternet\\{}",
        APP_NAME
    ));

    // URLハンドラ削除
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let _ = hkcr.delete_subkey_all(format!("{}URL", APP_NAME));

    Ok(())
}