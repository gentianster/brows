#![windows_subsystem = "windows"]

mod browser;
mod config;
mod icon;
mod ipc;
mod lang;
mod registry;
mod ui;
mod updater;
mod util;

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--register") => {
            registry::register()?;
            util::spawn_self_detached(&[]);
        }
        Some("--unregister") => {
            registry::unregister()?;
            util::spawn_self_detached(&[]);
        }
        Some("--resident") => {
            // スタートアップ登録から呼ばれる。ウィンドウを作らず（非表示のまま）常駐する
            ui::run_resident()?;
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