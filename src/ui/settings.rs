//! 設定画面（既定ブラウザ登録・URL ルール・自動更新）

use super::win32::center_pos;
use super::{app_icon, setup_fonts};
use crate::browser::{self, BackgroundDetect, BrowserGroup};
use crate::config::{Config, Rule};
use crate::registry;
use crate::updater::{UpdateState, Updater};
use anyhow::Result;
use eframe::egui;
use std::collections::HashMap;

pub fn show_settings() -> Result<()> {
    let lang = crate::lang::get();
    let config = Config::load().unwrap_or_default();

    let groups = if !config.cached_groups.is_empty() {
        config.cached_groups.clone()
    } else {
        let g = browser::detect_grouped().unwrap_or_default();
        let _ = Config::update(|cfg| cfg.cached_groups = g.clone());
        g
    };

    let detect = BackgroundDetect::new();
    detect.spawn();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(lang.window_title_settings)
        .with_inner_size([480.0, 520.0])
        .with_position(center_pos(480.0, 520.0))
        .with_resizable(true);
    if let Some(icon) = app_icon() { viewport = viewport.with_icon(icon); }
    let options = eframe::NativeOptions { viewport, ..Default::default() };

    eframe::run_native(
        "brows settings",
        options,
        Box::new(move |cc| {
            setup_fonts(cc);
            Box::new(SettingsApp::new(groups, config, detect))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct SettingsApp {
    groups: Vec<BrowserGroup>,
    registered: bool,
    startup: bool,
    status_msg: Option<String>,
    updater: Updater,
    config: Config,
    new_pattern: String,
    new_browser: String,
    icons: HashMap<String, egui::TextureHandle>,
    icons_loaded: bool,
    rule_search: String,
    detect: BackgroundDetect,
}

impl SettingsApp {
    fn new(groups: Vec<BrowserGroup>, config: Config, detect: BackgroundDetect) -> Self {
        let registered = registry::is_registered();
        let startup = registry::is_startup_registered();
        let updater = Updater::from_config(&config);
        let new_browser = groups.first()
            .and_then(|g| g.browsers.first())
            .map(|b| b.name.clone())
            .unwrap_or_default();
        Self {
            groups, registered, startup, status_msg: None, updater, config,
            new_pattern: String::new(), new_browser,
            icons: HashMap::new(), icons_loaded: false,
            rule_search: String::new(),
            detect,
        }
    }

    /// ルール変更を保存する（他フィールドを巻き戻さないよう rules だけ書く）
    fn save_rules(&self) {
        let rules = self.config.rules.clone();
        let _ = Config::update(|cfg| cfg.rules = rules);
    }

    fn browser_names(&self) -> Vec<String> {
        self.groups.iter().flat_map(|g| {
            if g.browsers.len() == 1 {
                vec![g.name.clone()]
            } else {
                g.browsers.iter().map(|b| b.name.clone()).collect()
            }
        }).collect()
    }
}

fn draw_x_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    let color = if resp.hovered() {
        egui::Color32::from_rgb(220, 80, 80)
    } else {
        ui.style().visuals.weak_text_color()
    };
    let c = rect.center();
    let d = 4.0;
    let stroke = egui::Stroke::new(1.5, color);
    ui.painter().line_segment([egui::pos2(c.x - d, c.y - d), egui::pos2(c.x + d, c.y + d)], stroke);
    ui.painter().line_segment([egui::pos2(c.x + d, c.y - d), egui::pos2(c.x - d, c.y + d)], stroke);
    resp
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let lang = crate::lang::get();

        // バックグラウンド検出が完了していたら表示を更新（キャッシュ保存は take 内で行われる）
        if let Some(fresh) = self.detect.take() {
            if browser::groups_differ(&fresh, &self.groups) {
                self.groups = fresh;
                self.icons.clear();
                self.icons_loaded = false;
            }
        }

        if !self.icons_loaded {
            self.icons_loaded = true;
            for g in &self.groups {
                if let Some(img) = crate::icon::load(&g.exe_path) {
                    let tex = ctx.load_texture(&g.name, img, egui::TextureOptions::LINEAR);
                    for b in &g.browsers {
                        self.icons.insert(b.name.clone(), tex.clone());
                    }
                    self.icons.insert(g.name.clone(), tex);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("brows");
                ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak().small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(lang.subtitle).weak().small());
                });
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                let (icon, label) = if self.registered {
                    ("✔", egui::RichText::new(lang.registered).color(egui::Color32::from_rgb(100, 200, 100)).small())
                } else {
                    ("✖", egui::RichText::new(lang.not_registered).color(egui::Color32::from_rgb(200, 100, 100)).small())
                };
                ui.label(icon);
                ui.label(label);
                ui.add_space(4.0);
                if ui.add_enabled(!self.registered, egui::Button::new(lang.btn_register).small()).clicked() {
                    match registry::register() {
                        Ok(_) => {
                            self.registered = true;
                            self.status_msg = Some(lang.register_success_hint.into());
                        }
                        Err(_) => {
                            registry::elevate("--register");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
                if ui.add_enabled(self.registered, egui::Button::new(lang.btn_unregister).small()).clicked() {
                    match registry::unregister() {
                        Ok(_) => {
                            self.registered = false;
                            self.status_msg = Some(lang.unregister_success.into());
                        }
                        Err(_) => {
                            registry::elevate("--unregister");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let update_state = self.updater.state.lock().unwrap().clone();
                    match &update_state {
                        UpdateState::UpToDate => {
                            if ui.small_button(lang.btn_check_update).clicked() {
                                self.updater.check_now();
                            }
                            ui.label(egui::RichText::new(lang.up_to_date).weak().small());
                        }
                        UpdateState::Checking => {
                            ui.label(egui::RichText::new(lang.checking).weak().small());
                            ctx.request_repaint();
                        }
                        UpdateState::Available(tag) => {
                            if ui.small_button(lang.btn_download).clicked() {
                                self.updater.download_and_restart();
                            }
                            ui.label(egui::RichText::new(format!("{} {}", tag, lang.update_suffix))
                                .color(egui::Color32::from_rgb(80, 180, 80)).small());
                        }
                        UpdateState::Downloading => {
                            ui.label(egui::RichText::new(lang.downloading).weak().small());
                            ctx.request_repaint();
                        }
                        UpdateState::ReadyToRestart => {
                            if ui.small_button(lang.btn_restart).clicked() { Updater::restart(); }
                            ui.label(egui::RichText::new(lang.dl_complete)
                                .color(egui::Color32::from_rgb(80, 180, 80)).small());
                        }
                        UpdateState::Error(e) => {
                            if ui.small_button(lang.btn_check_update).clicked() {
                                self.updater.check_now();
                            }
                            ui.label(egui::RichText::new(format!("{}{}", lang.update_error_prefix, e))
                                .color(egui::Color32::from_rgb(200, 80, 80)).small());
                        }
                    }
                });
            });

            if let Some(msg) = &self.status_msg {
                ui.label(egui::RichText::new(msg).weak().small());
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut startup = self.startup;
                if ui.checkbox(&mut startup, egui::RichText::new(lang.startup_checkbox).small()).changed() {
                    let result = if startup {
                        registry::register_startup()
                    } else {
                        registry::unregister_startup()
                    };
                    if result.is_ok() {
                        self.startup = startup;
                        if startup {
                            // 次のログオンを待たず、いますぐ常駐を開始する
                            // （既に常駐がいれば即終了するので無害）
                            crate::util::spawn_self_detached(&["--resident"]);
                        }
                    }
                }
                ui.label(egui::RichText::new(lang.startup_hint).weak().small());
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(lang.section_url_rules);
                if ui.small_button(lang.btn_open_config).clicked() {
                    use std::os::windows::process::CommandExt;
                    let path = crate::config::config_path();
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if !path.exists() { let _ = std::fs::write(&path, ""); }
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &path.to_string_lossy()])
                        .creation_flags(crate::util::CREATE_NO_WINDOW)
                        .spawn();
                }
            });
            ui.add_space(4.0);

            let browser_names = self.browser_names();
            let mut delete_idx: Option<usize> = None;
            if self.config.rules.is_empty() {
                ui.label(egui::RichText::new(lang.no_rules).weak().small());
            } else {
                ui.add(egui::TextEdit::singleline(&mut self.rule_search)
                    .hint_text(lang.search_hint)
                    .desired_width(f32::INFINITY));
                ui.add_space(2.0);
                let q = self.rule_search.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_source("rules_scroll")
                    .max_height(120.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (i, rule) in self.config.rules.iter().enumerate()
                            .filter(|(_, r)| q.is_empty()
                                || r.pattern.to_lowercase().contains(&q)
                                || r.browser.to_lowercase().contains(&q))
                        {
                            ui.horizontal(|ui| {
                                if draw_x_button(ui).clicked() {
                                    delete_idx = Some(i);
                                }
                                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                if let Some(tex) = self.icons.get(&rule.browser) {
                                    ui.painter().image(tex.id(), icon_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE);
                                }
                                ui.label(egui::RichText::new(&rule.pattern).monospace());
                                ui.label(egui::RichText::new("→").weak());
                                ui.label(&rule.browser);
                            });
                        }
                    });
            }
            if let Some(i) = delete_idx {
                self.config.rules.remove(i);
                self.save_rules();
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_pattern)
                    .hint_text(lang.pattern_hint)
                    .desired_width(160.0));
                egui::ComboBox::from_id_source("rule_browser")
                    .selected_text(&self.new_browser)
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for name in &browser_names {
                            let is_selected = self.new_browser == *name;
                            let resp = ui.horizontal(|ui| {
                                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                if let Some(tex) = self.icons.get(name) {
                                    ui.painter().image(tex.id(), icon_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE);
                                }
                                ui.selectable_label(is_selected, name.as_str())
                            });
                            if resp.inner.clicked() {
                                self.new_browser = name.clone();
                            }
                        }
                    });
                let can_add = !self.new_pattern.is_empty() && !self.new_browser.is_empty();
                if ui.add_enabled(can_add, egui::Button::new(lang.btn_add)).clicked() {
                    self.config.rules.push(Rule {
                        pattern: self.new_pattern.clone(),
                        browser: self.new_browser.clone(),
                    });
                    self.save_rules();
                    self.new_pattern.clear();
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(lang.section_browsers);
            ui.add_space(4.0);

            if self.groups.is_empty() {
                ui.label(egui::RichText::new(lang.no_browsers).weak());
            } else {
                for g in &self.groups {
                    ui.horizontal(|ui| {
                        ui.label(&g.name);
                        ui.label(egui::RichText::new(&g.exe_path).weak().small());
                    });
                }
            }
        });
    }
}
