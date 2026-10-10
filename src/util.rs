/// コンソールウィンドウを出さずに子プロセスを起動する (CreateProcess フラグ)
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// 親プロセスから切り離して起動する (CreateProcess フラグ)
#[cfg(windows)]
pub const DETACHED_PROCESS: u32 = 0x0000_0008;

/// 自分自身の exe を切り離したプロセスとして起動する
pub fn spawn_self_detached(args: &[&str]) {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    if let Ok(exe) = std::env::current_exe() {
        let mut command = std::process::Command::new(exe);
        command.args(args);
        #[cfg(windows)]
        command.creation_flags(DETACHED_PROCESS);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            use std::process::Stdio;
            command
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        }
        let _ = command.spawn();
    }
}

pub fn open_file(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

/// 管理者権限（昇格トークン）で動作しているか。
/// 昇格した常駐は通常権限のリンク起動からパイプ越しに扱いにくいため、
/// 昇格時はトレイ常駐の自動確保を避ける判定に使う。
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
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

#[cfg(target_os = "linux")]
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// JSON テキストから `"key": "value"` の value を返す（簡易・ネストなし）
#[cfg(windows)]
pub fn json_str(text: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\":", key);
    let pos = text.find(&search)?;
    let after = text[pos + search.len()..].trim_start();
    let value = after.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}
