#![windows_subsystem = "windows"]

mod browser;
mod config;
mod icon;
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
            println!("brows を既定ブラウザとして登録しました。");
            println!("設定 → アプリ → 既定のアプリ から brows を既定ブラウザに設定してください。");
        }
        Some("--unregister") => {
            registry::unregister()?;
            println!("登録を解除しました。");
        }
        Some("--list") => {
            let browsers = browser::detect()?;
            for b in &browsers {
                println!("{} : {}", b.name, b.exe_path);
            }
        }
        Some(url) if url.starts_with("http") => {
            // 既定ブラウザとして呼び出された場合
            ui::show_picker(url.to_string())?;
        }
        _ => {
            ui::show_settings()?;
        }
    }

    Ok(())
}