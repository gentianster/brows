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
    icons: Vec<Option<egui::TextureHandle>>,
    icons_loaded: bool,
}

impl PickerApp {
    fn new(url: String, browsers: Vec<Browser>) -> Self {
        let n = browsers.len();
        Self { url, browsers, selected: None, icons: vec![None; n], icons_loaded: false }
    }
}

impl eframe::App for PickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 初回フレームでアイコンをロード
        if !self.icons_loaded {
            self.icons_loaded = true;
            for (i, b) in self.browsers.iter().enumerate() {
                if let Some(img) = crate::icon::load(&b.exe_path) {
                    self.icons[i] = Some(ctx.load_texture(
                        &b.name,
                        img,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
        }

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

            const ROW_H: f32 = 40.0;
            const ICON_SIZE: f32 = 22.0;

            for (i, browser) in self.browsers.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_H),
                    egui::Sense::click(),
                );
                let visuals = ui.style().interact(&response);
                ui.painter().rect(rect, 4.0, visuals.bg_fill, visuals.bg_stroke);

                // アイコン
                if let Some(Some(tex)) = self.icons.get(i) {
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.min.x + 10.0, rect.center().y - ICON_SIZE / 2.0),
                        egui::vec2(ICON_SIZE, ICON_SIZE),
                    );
                    ui.painter().image(
                        tex.id(),
                        icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                // ブラウザ名（中央揃え）
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &browser.name,
                    egui::FontId::proportional(14.0),
                    visuals.text_color(),
                );

                if response.clicked() {
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
