use crate::browser::{self, Browser};
use crate::config::Config;
use anyhow::Result;
use eframe::egui;

pub fn show_picker(url: String) -> Result<()> {
    let browsers = browser::detect()?;
    let config = Config::load()?;

    // ルールにマッチしたら即起動（UIを出さない）
    if let Some(browser_name) = config.match_rule(&url) {
        if let Some(b) = browsers.iter().find(|b| b.name == browser_name) {
            return b.launch(&url);
        }
    }

    // デフォルトブラウザが設定されていたら即起動
    if let Some(default) = &config.default_browser {
        if let Some(b) = browsers.iter().find(|b| &b.name == default) {
            return b.launch(&url);
        }
    }

    // ブラウザ選択UIを表示
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
            Box::new(BrowsApp::new(url, browsers))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct BrowsApp {
    url: String,
    browsers: Vec<Browser>,
    selected: Option<usize>,
}

impl BrowsApp {
    fn new(url: String, browsers: Vec<Browser>) -> Self {
        Self { url, browsers, selected: None }
    }
}

impl eframe::App for BrowsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // URL表示（長い場合は省略）
            let display_url = if self.url.len() > 50 {
                format!("{}...", &self.url[..50])
            } else {
                self.url.clone()
            };
            ui.label(egui::RichText::new(&display_url).weak().small());
            ui.add_space(12.0);

            ui.label("どのブラウザで開きますか？");
            ui.add_space(8.0);

            // ブラウザ一覧ボタン
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

            // キャンセル
            if ui.button("キャンセル").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        // 選択済みならブラウザ起動
        if let Some(i) = self.selected {
            if let Some(b) = self.browsers.get(i) {
                let _ = b.launch(&self.url);
                self.selected = None;
            }
        }
    }
}