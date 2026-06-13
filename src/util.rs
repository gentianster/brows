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

/// JSON テキストから `"key": "value"` の value を返す（簡易・ネストなし）
pub fn json_str(text: &str, key: &str) -> Option<String> {
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
