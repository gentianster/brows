#![windows_subsystem = "windows"]

mod browser;
mod config;
mod icon;
mod ipc;
mod lang;
mod registry;
mod ui;
mod updater;

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--register") => {
            registry::register()?;
            relaunch_settings();
        }
        Some("--unregister") => {
            registry::unregister()?;
            relaunch_settings();
        }
        Some("--list") => {
            let browsers = browser::detect()?;
            for b in &browsers {
                println!("{} : {}", b.name, b.exe_path);
            }
        }
        Some(url) if url.starts_with("http") => {
            // 更新チェックは常駐側で行う（転送だけで即終了するプロセスで
            // 走らせるとチェック完了前に殺されてしまうため）
            ui::open_url(url.to_string())?;
        }
        _ => {
            ui::show_settings()?;
        }
    }

    Ok(())
}

pub fn relaunch_settings() {
    use std::os::windows::process::CommandExt;
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .creation_flags(0x00000008) // DETACHED_PROCESS
            .spawn();
    }
}