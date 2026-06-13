/// コンソールウィンドウを出さずに子プロセスを起動する (CreateProcess フラグ)
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// 親プロセスから切り離して起動する (CreateProcess フラグ)
pub const DETACHED_PROCESS: u32 = 0x0000_0008;

/// 自分自身の exe を切り離したプロセスとして起動する
pub fn spawn_self_detached(args: &[&str]) {
    use std::os::windows::process::CommandExt;
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .args(args)
            .creation_flags(DETACHED_PROCESS)
            .spawn();
    }
}

/// 管理者権限（昇格トークン）で動作しているか。
/// 昇格した常駐は通常権限のリンク起動からパイプ越しに扱いにくいため、
/// 昇格時はトレイ常駐の自動確保を避ける判定に使う。
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// JSON テキストから `"key": "value"` の value を返す（簡易・ネストなし）
pub fn json_str(text: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\":", key);
    let pos = text.find(&search)?;
    let after = text[pos + search.len()..].trim_start();
    let value = after.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}
