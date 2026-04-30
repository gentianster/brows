use crate::browser::{self, Browser, BrowserGroup};
use crate::config::Config;
use crate::registry;
use crate::updater::{UpdateState, Updater};
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
    let mut groups = browser::detect_grouped()?;
    let config = Config::load()?;

    // 保存された順序で並べ替え
    if !config.browser_order.is_empty() {
        groups.sort_by_key(|g| {
            config.browser_order.iter().position(|o| o == &g.exe_path).unwrap_or(usize::MAX)
        });
    }

    // ルール・デフォルトブラウザのチェック
    for g in &groups {
        for b in &g.browsers {
            if config.match_rule(&url).map_or(false, |n| n == b.name) {
                return b.launch(&url);
            }
        }
    }
    if let Some(default) = &config.default_browser {
        for g in &groups {
            if let Some(b) = g.browsers.iter().find(|b| &b.name == default) {
                return b.launch(&url);
            }
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
            Box::new(PickerApp::new(url, groups, config))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct PickerApp {
    url: String,
    groups: Vec<BrowserGroup>,
    config: Config,
    expanded: Option<usize>,
    selected: Option<(usize, usize)>,
    icons: Vec<Option<egui::TextureHandle>>,
    icons_loaded: bool,
    // ドラッグ状態
    drag_src: Option<usize>,
    drag_tgt: usize,
    row_rects: Vec<egui::Rect>,
}

impl PickerApp {
    fn new(url: String, groups: Vec<BrowserGroup>, config: Config) -> Self {
        let n = groups.len();
        Self {
            url, groups, config,
            expanded: None, selected: None,
            icons: vec![None; n], icons_loaded: false,
            drag_src: None, drag_tgt: 0,
            row_rects: vec![egui::Rect::NOTHING; n],
        }
    }

    fn save_order(&mut self) {
        self.config.browser_order = self.groups.iter().map(|g| g.exe_path.clone()).collect();
        let _ = self.config.save();
    }
}

fn draw_drop_line(ui: &egui::Ui, x_min: f32, x_max: f32, y: f32) {
    let color = egui::Color32::from_rgb(80, 140, 255);
    ui.painter().line_segment(
        [egui::pos2(x_min, y), egui::pos2(x_max, y)],
        egui::Stroke::new(2.0, color),
    );
    ui.painter().circle_filled(egui::pos2(x_min + 6.0, y), 3.0, color);
}

impl eframe::App for PickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.icons_loaded {
            self.icons_loaded = true;
            for (i, g) in self.groups.iter().enumerate() {
                if let Some(img) = crate::icon::load(&g.exe_path) {
                    self.icons[i] = Some(ctx.load_texture(&g.name, img, egui::TextureOptions::LINEAR));
                }
            }
        }

        // 前フレームの rect からドロップ先を計算
        if self.drag_src.is_some() {
            if let Some(py) = ctx.input(|i| i.pointer.hover_pos()).map(|p| p.y) {
                let src = self.drag_src.unwrap();
                let mut tgt = self.row_rects.len();
                for (i, r) in self.row_rects.iter().enumerate() {
                    if py < r.center().y {
                        tgt = i;
                        break;
                    }
                }
                // src の前後は実質移動なしなので現在位置のまま
                if tgt == src || tgt == src + 1 { tgt = src; }
                self.drag_tgt = tgt;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            let display_url = if self.url.len() > 50 {
                format!("{}...", &self.url[..50])
            } else { self.url.clone() };
            ui.label(egui::RichText::new(&display_url).weak().small());
            ui.add_space(12.0);
            ui.label("どのブラウザで開きますか？");
            ui.add_space(8.0);

            const ROW_H: f32 = 40.0;
            const PROFILE_H: f32 = 34.0;
            const ICON_SIZE: f32 = 22.0;
            let w = ui.available_width();
            let x_min = ui.cursor().min.x;

            let is_dragging = self.drag_src.is_some();
            let mut drop_performed = false;

            for gi in 0..self.groups.len() {
                let is_expanded = self.expanded == Some(gi);
                let is_expandable = self.groups[gi].browsers.len() > 1;
                let is_drag_src = self.drag_src == Some(gi);

                // ドロップ先インジケーター（このアイテムの上）
                if is_dragging && self.drag_tgt == gi && self.drag_tgt != self.drag_src.unwrap_or(usize::MAX) {
                    let y = ui.cursor().min.y + 1.0;
                    draw_drop_line(ui, x_min, x_min + w, y);
                    ui.add_space(4.0);
                }

                // ── グループ行 ──
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(w, ROW_H), egui::Sense::hover(),
                );
                if gi < self.row_rects.len() { self.row_rects[gi] = rect; }

                let resp = ui.interact(rect, ui.id().with(gi), egui::Sense::click_and_drag());
                let visuals = ui.style().interact(&resp);

                let bg = if is_drag_src {
                    egui::Color32::from_rgba_unmultiplied(
                        visuals.bg_fill.r(), visuals.bg_fill.g(), visuals.bg_fill.b(), 80)
                } else { visuals.bg_fill };
                ui.painter().rect(rect, 4.0, bg, visuals.bg_stroke);

                // アイコン
                if let Some(Some(tex)) = self.icons.get(gi) {
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.min.x + 10.0, rect.center().y - ICON_SIZE / 2.0),
                        egui::vec2(ICON_SIZE, ICON_SIZE),
                    );
                    ui.painter().image(tex.id(), icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }

                // ブラウザ名
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                    &self.groups[gi].name, egui::FontId::proportional(14.0), visuals.text_color());

                // 展開三角 or ドラッグハンドル
                if is_expandable {
                    let cx = rect.max.x - 16.0;
                    let cy = rect.center().y;
                    let pts = if is_expanded {
                        vec![egui::pos2(cx-6.0,cy-3.0), egui::pos2(cx+6.0,cy-3.0), egui::pos2(cx,cy+4.0)]
                    } else {
                        vec![egui::pos2(cx-3.0,cy-6.0), egui::pos2(cx-3.0,cy+6.0), egui::pos2(cx+4.0,cy)]
                    };
                    ui.painter().add(egui::Shape::convex_polygon(pts, visuals.text_color(), egui::Stroke::NONE));
                }

                // イベント処理
                if resp.drag_started() {
                    self.drag_src = Some(gi);
                    self.drag_tgt = gi;
                    self.expanded = None;
                }
                if resp.drag_stopped() {
                    if let Some(src) = self.drag_src.take() {
                        let tgt = self.drag_tgt;
                        if tgt != src && tgt != src + 1 {
                            let item = self.groups.remove(src);
                            let insert = if tgt > src { tgt - 1 } else { tgt };
                            self.groups.insert(insert, item);
                            // アイコンも同様に並べ替え
                            let icon = self.icons.remove(src);
                            let icon_insert = if tgt > src { tgt - 1 } else { tgt };
                            self.icons.insert(icon_insert, icon);
                            self.save_order();
                            drop_performed = true;
                        }
                    }
                }
                if resp.clicked() && !is_dragging {
                    if is_expandable {
                        self.expanded = if is_expanded { None } else { Some(gi) };
                    } else {
                        self.selected = Some((gi, 0));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                // ── プロファイル行（展開時） ──
                if is_expanded && !drop_performed {
                    for (pi, browser) in self.groups[gi].browsers.iter().enumerate() {
                        let (prect, presp) = ui.allocate_exact_size(
                            egui::vec2(w, PROFILE_H), egui::Sense::click());
                        let pvis = ui.style().interact(&presp);
                        ui.painter().rect(prect, 0.0, pvis.bg_fill, pvis.bg_stroke);
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(prect.min, egui::vec2(3.0, prect.height())),
                            0.0, egui::Color32::from_rgb(80, 120, 200));
                        ui.painter().text(
                            egui::pos2(prect.min.x + 20.0, prect.center().y),
                            egui::Align2::LEFT_CENTER, &browser.name,
                            egui::FontId::proportional(13.0), pvis.text_color());
                        if presp.clicked() {
                            self.selected = Some((gi, pi));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }

            // リスト末尾へのドロップインジケーター
            let last = self.groups.len();
            if is_dragging && self.drag_tgt == last && self.drag_src != Some(last.saturating_sub(1)) {
                let y = ui.cursor().min.y + 1.0;
                draw_drop_line(ui, x_min, x_min + w, y);
                ui.add_space(4.0);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            if ui.button("キャンセル").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        if let Some((gi, pi)) = self.selected {
            if let Some(b) = self.groups.get(gi).and_then(|g| g.browsers.get(pi)) {
                let _ = b.launch(&self.url);
            }
            self.selected = None;
        }
    }
}

// ─── 設定画面 ────────────────────────────────────────────────────

pub fn show_settings() -> Result<()> {
    let browsers = browser::detect().unwrap_or_default();
    let config = Config::load().unwrap_or_default();

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
        Box::new(move |cc| {
            setup_fonts(cc);
            Box::new(SettingsApp::new(browsers, &config))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct SettingsApp {
    browsers: Vec<Browser>,
    registered: bool,
    status_msg: Option<String>,
    updater: Updater,
}

impl SettingsApp {
    fn new(browsers: Vec<Browser>, config: &Config) -> Self {
        let registered = is_registered();
        Self { browsers, registered, status_msg: None, updater: Updater::from_config(config) }
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
                        Err(_) => {
                            registry::elevate("--register");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
                if ui.add_enabled(self.registered, egui::Button::new("登録解除")).clicked() {
                    match registry::unregister() {
                        Ok(_) => { self.registered = false; self.status_msg = Some("登録を解除しました。".into()); }
                        Err(_) => {
                            registry::elevate("--unregister");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

            let update_state = self.updater.state.lock().unwrap().clone();
            match &update_state {
                UpdateState::UpToDate => {
                    ui.label(egui::RichText::new("最新バージョンです").weak().small());
                }
                UpdateState::Available(tag) => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("新バージョン {} があります", tag))
                            .color(egui::Color32::from_rgb(80, 180, 80)));
                        if ui.button("ダウンロード & 再起動").clicked() {
                            self.updater.download_and_restart();
                        }
                    });
                }
                UpdateState::Downloading => {
                    ui.label(egui::RichText::new("ダウンロード中...").weak().small());
                    ctx.request_repaint();
                }
                UpdateState::ReadyToRestart => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("ダウンロード完了").color(egui::Color32::from_rgb(80, 180, 80)));
                        if ui.button("今すぐ再起動").clicked() {
                            Updater::restart();
                        }
                    });
                }
                UpdateState::Error(e) => {
                    ui.label(egui::RichText::new(format!("更新エラー: {}", e))
                        .color(egui::Color32::from_rgb(200, 80, 80)).small());
                }
            }

            ui.add_space(8.0);
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
