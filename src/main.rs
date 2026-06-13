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
            // どの起動経路でもトレイ常駐を確保する（--resident は冪等：既に常駐が
            // いれば即終了する）。ただし管理者権限のときは常駐を作らない
            // （昇格した常駐は通常権限のリンク起動からパイプ越しに扱いにくく、
            // --register 直後などに不整合を生むため）。
            if !util::is_elevated() {
                util::spawn_self_detached(&["--resident"]);
            }
            ui::show_settings()?;
        }
    }

    Ok(())
}