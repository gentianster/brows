use crate::browser::{self, Browser};
use crate::config::Config;
use crate::registry;
use anyhow::Result;
use eframe::egui;

fn setup_fonts(cc: &eframe::CreationContext) {
    let mut fonts = egui::FontDefinitions::default();
    let font_candidates = [
        "C:\\Windows\\Fonts\\YuGothM.ttc",
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
    ];
    for path in &font_candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("ja".to_owned(), egui::FontData::from_owned(data));
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().push("ja".to_owned());
            break;
        }
    }
    cc.egui_ctx.set_fonts(fonts);
}

// ─── ブラウザ選択ピッカー ────────────────────────────────────────

pub fn show_picker(url: String) -> Result<()> {
    let browsers = browser::detect()?;
    let config = Config::load()?;

    if let Some(browser_name) = config.match_rule(&url) {
        if let Some(b) = browsers.iter().find(|b| b.name == browser_name) {
            return b.launch(&url);
        }
    }

    if let Some(default) = &config.default_browser {
        if let Some(b) = browsers.iter().find(|b| &b.name == default) {
            return b.launch(&url);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("brows")
            .with_inner_size([400.0, 300.0])
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "brows",
        options,
        Box::new(|cc| {
            setup_fonts(cc);
            Box::new(PickerApp::new(url, browsers))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct PickerApp {
    url: String,
    browsers: Vec<Browser>,
    selected: Option<usize>,
}

impl PickerApp {
    fn new(url: String, browsers: Vec<Browser>) -> Self {
        Self { url, browsers, selected: None }
    }
}

impl eframe::App for PickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            let display_url = if self.url.len() > 50 {
                format!("{}...", &self.url[..50])
            } else {
                self.url.clone()
            };
            ui.label(egui::RichText::new(&display_url).weak().small());
            ui.add_space(12.0);

            ui.label("どのブラウザで開きますか？");
            ui.add_space(8.0);

            for (i, browser) in self.browsers.iter().enumerate() {
                let btn = ui.add_sized(
                    [ui.available_width(), 36.0],
                    egui::Button::new(&browser.name),
                );
                if btn.clicked() {
                    self.selected = Some(i);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            if ui.button("キャンセル").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        if let Some(i) = self.selected {
            if let Some(b) = self.browsers.get(i) {
                let _ = b.launch(&self.url);
                self.selected = None;
            }
        }
    }
}

// ─── 設定画面 ────────────────────────────────────────────────────

pub fn show_settings() -> Result<()> {
    let browsers = browser::detect().unwrap_or_default();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("brows - 設定")
            .with_inner_size([440.0, 380.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "brows settings",
        options,
        Box::new(|cc| {
            setup_fonts(cc);
            Box::new(SettingsApp::new(browsers))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct SettingsApp {
    browsers: Vec<Browser>,
    registered: bool,
    status_msg: Option<String>,
}

impl SettingsApp {
    fn new(browsers: Vec<Browser>) -> Self {
        let registered = is_registered();
        Self { browsers, registered, status_msg: None }
    }
}

fn is_registered() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Clients\\StartMenuInternet\\brows")
        .is_ok()
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("brows");
            ui.label(egui::RichText::new("ブラウザ選択ツール for Windows 11").weak());
            ui.add_space(16.0);

            ui.separator();
            ui.add_space(8.0);

            // 登録状態
            ui.horizontal(|ui| {
                let (icon, label) = if self.registered {
                    ("✔", egui::RichText::new("既定ブラウザとして登録済み").color(egui::Color32::from_rgb(100, 200, 100)))
                } else {
                    ("✖", egui::RichText::new("未登録").color(egui::Color32::from_rgb(200, 100, 100)))
                };
                ui.label(icon);
                ui.label(label);
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add_enabled(!self.registered, egui::Button::new("既定ブラウザとして登録")).clicked() {
                    match registry::register() {
                        Ok(_) => {
                            self.registered = true;
                            self.status_msg = Some("登録しました。設定 → アプリ → 既定のアプリ から brows を選択してください。".into());
                        }
                        Err(e) => {
                            self.status_msg = Some(format!("登録失敗: {} (管理者権限で実行してください)", e));
                        }
                    }
                }

                if ui.add_enabled(self.registered, egui::Button::new("登録解除")).clicked() {
                    match registry::unregister() {
                        Ok(_) => {
                            self.registered = false;
                            self.status_msg = Some("登録を解除しました。".into());
                        }
                        Err(e) => {
                            self.status_msg = Some(format!("解除失敗: {}", e));
                        }
                    }
                }
            });

            if let Some(msg) = &self.status_msg {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(msg).weak().small());
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label("検出済みブラウザ");
            ui.add_space(4.0);

            if self.browsers.is_empty() {
                ui.label(egui::RichText::new("ブラウザが見つかりませんでした").weak());
            } else {
                for b in &self.browsers {
                    ui.horizontal(|ui| {
                        ui.label(&b.name);
                        ui.label(egui::RichText::new(&b.exe_path).weak().small());
                    });
                }
            }
        });
    }
}
