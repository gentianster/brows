use anyhow::Result;
use std::os::windows::ffi::OsStrExt;
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

/// ブラウザとして登録済みかどうか
pub fn is_registered() -> bool {
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!("SOFTWARE\\Clients\\StartMenuInternet\\{}", APP_NAME))
        .is_ok()
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// ログオン時に常駐ピッカーを起動するスタートアップ登録（HKCU なので管理者権限不要）
pub fn register_startup() -> Result<()> {
    let exe_path = std::env::current_exe()?.to_string_lossy().to_string();
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    key.set_value(APP_NAME, &format!("\"{}\" --resident", exe_path))?;
    Ok(())
}

/// スタートアップ登録を解除する
pub fn unregister_startup() -> Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    key.delete_value(APP_NAME)?;
    Ok(())
}

pub fn is_startup_registered() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
}

/// UAC 昇格して brows.exe <arg> を再実行する
pub fn elevate(arg: &str) {
    use std::ffi::OsStr;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let Ok(exe) = std::env::current_exe() else { return };
    let to_wide = |s: &OsStr| -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    };
    let verb  = to_wide(OsStr::new("runas"));
    let file  = to_wide(exe.as_os_str());
    let param = to_wide(OsStr::new(arg));

    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(param.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// 登録を解除する
pub fn unregister() -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 書き込み権限チェック（失敗時は Err を返して UAC 昇格を促す）
    hklm.open_subkey_with_flags("SOFTWARE\\Clients\\StartMenuInternet", KEY_WRITE)?;

    if let Ok(reg_apps) = hklm.open_subkey_with_flags("SOFTWARE\\RegisteredApplications", KEY_WRITE) {
        let _ = reg_apps.delete_value(APP_NAME);
    }
    let _ = hklm.delete_subkey_all(format!("SOFTWARE\\Clients\\StartMenuInternet\\{}", APP_NAME));

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let _ = hkcr.delete_subkey_all(format!("{}URL", APP_NAME));

    Ok(())
}