//! ブラウザ選択ピッカー。常駐インスタンスとして IPC リクエストも受け付ける

use super::win32::{center_pos, force_hide, force_show, hwnd_of};
use super::{app_icon, setup_fonts};
use crate::browser::{self, BackgroundDetect, Browser, BrowserGroup};
use crate::config::Config;
use anyhow::Result;
use eframe::egui;
use std::sync::{Arc, Mutex};

/// ルール → 既定ブラウザの順で、UI を出さずに自動起動すべきブラウザを返す
fn find_auto_browser<'a>(groups: &'a [BrowserGroup], config: &Config, url: &str) -> Option<&'a Browser> {
    config
        .match_rule(url)
        .into_iter()
        .chain(config.default_browser.as_deref())
        .find_map(|name| {
            groups
                .iter()
                .find_map(|g| g.browsers.iter().find(|b| b.name == name))
        })
}

/// URL を開く。常駐インスタンスがいれば転送し、いなければ自分が常駐になる
pub fn open_url(url: String) -> Result<()> {
    // 高速パス: 常駐インスタンスへ転送して即終了
    if crate::ipc::send_open(&url) {
        return Ok(());
    }
    match crate::ipc::try_bind() {
        Some(listener) => show_picker(Some(url), Some(listener)),
        None => {
            // ポートは塞がっているが転送に失敗した。起動直後の競合の
            // 可能性があるので一度だけ再試行し、ダメなら常駐なしで表示する
            if crate::ipc::send_open(&url) {
                return Ok(());
            }
            show_picker(Some(url), None)
        }
    }
}

/// スタートアップ登録から呼ばれる。ウィンドウを表示せずに常駐だけ始める
pub fn run_resident() -> Result<()> {
    match crate::ipc::try_bind() {
        Some(listener) => show_picker(None, Some(listener)),
        None => Ok(()), // 既に常駐がいる（またはポートが使えない）ので何もしない
    }
}

fn show_picker(url: Option<String>, listener: Option<std::net::TcpListener>) -> Result<()> {
    let config = Config::load()?;

    // キャッシュがあれば即使用、なければ初回のみ同期検出してキャッシュ保存
    let mut groups = if !config.cached_groups.is_empty() {
        config.cached_groups.clone()
    } else {
        let g = browser::detect_grouped()?;
        let _ = Config::update(|cfg| cfg.cached_groups = g.clone());
        g
    };
    config.sort_groups(&mut groups);

    // バックグラウンドで再検出してキャッシュを更新
    let detect = BackgroundDetect::new();
    detect.spawn();

    if let Some(u) = &url {
        if let Some(b) = find_auto_browser(&groups, &config, u) {
            return b.launch(u);
        }
    }

    // UI が動いている間にバックグラウンドで更新チェックを済ませる
    crate::updater::check_if_due();

    // URL なし（スタートアップ起動）は非表示で常駐する。eframe は初回描画後に
    // 無条件で set_visible(true) するため、画面外に配置した上で
    // 初回描画後のフレームで Win32 により隠す（PickerApp::hide_countdown）
    let start_hidden = url.is_none();
    let position = if start_hidden {
        egui::pos2(-30000.0, -30000.0)
    } else {
        center_pos(400.0, 300.0)
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("brows")
        .with_inner_size([400.0, 300.0])
        .with_position(position)
        .with_resizable(false)
        .with_always_on_top()
        .with_visible(!start_hidden);
    if let Some(icon) = app_icon() { viewport = viewport.with_icon(icon); }
    let options = eframe::NativeOptions { viewport, ..Default::default() };

    let resident = listener.is_some();
    // 常駐になるときだけタスクトレイにアイコンを出す（設定画面への導線）
    if resident {
        super::tray::spawn();
    }
    let incoming: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let incoming_clone = incoming.clone();

    eframe::run_native(
        "brows",
        options,
        Box::new(move |cc| {
            setup_fonts(cc);
            let hwnd = hwnd_of(cc);
            if let Some(listener) = listener {
                spawn_ipc_server(listener, cc.egui_ctx.clone(), incoming_clone, hwnd);
            }
            Box::new(PickerApp::new(url.unwrap_or_default(), groups, config, detect, resident, incoming, hwnd))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

/// 常駐モード: 別プロセスからのリクエストを受け付けるスレッドを起動する
fn spawn_ipc_server(
    listener: std::net::TcpListener,
    ctx: egui::Context,
    incoming: Arc<Mutex<Option<String>>>,
    hwnd: Option<isize>,
) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            match crate::ipc::read_request(stream) {
                Some(crate::ipc::Request::Open(url)) => {
                    crate::updater::check_if_due();
                    // ルール・既定ブラウザにマッチしたら UI を出さず直接起動
                    let auto = Config::load()
                        .ok()
                        .and_then(|cfg| find_auto_browser(&cfg.cached_groups, &cfg, &url).cloned());
                    if let Some(b) = auto {
                        let _ = b.launch(&url);
                        continue;
                    }
                    *incoming.lock().unwrap() = Some(url);
                    // 非表示ウィンドウは再描画イベントを受け取れないため、
                    // egui のコマンドではなく Win32 API で直接再表示する
                    if let Some(h) = hwnd {
                        force_show(h);
                    }
                    ctx.request_repaint();
                }
                Some(crate::ipc::Request::Exit) => {
                    super::tray::cleanup();
                    std::process::exit(0);
                }
                None => {}
            }
        }
    });
}

struct PickerApp {
    url: String,
    groups: Vec<BrowserGroup>,
    config: Config,
    expanded: Option<usize>,
    selected: Option<(usize, usize)>,
    icons: Vec<Option<egui::TextureHandle>>,
    icons_loaded: bool,
    drag_src: Option<usize>,
    drag_tgt: usize,
    row_rects: Vec<egui::Rect>,
    detect: BackgroundDetect,
    resident: bool,
    incoming: Arc<Mutex<Option<String>>>,
    hwnd: Option<isize>,
    /// スタートアップ起動時、eframe が初回描画後に強制表示するのを打ち消す
    /// ための残りフレーム数（0 になったフレームで Win32 により隠す）
    hide_countdown: u8,
}

impl PickerApp {
    fn new(
        url: String,
        groups: Vec<BrowserGroup>,
        config: Config,
        detect: BackgroundDetect,
        resident: bool,
        incoming: Arc<Mutex<Option<String>>>,
        hwnd: Option<isize>,
    ) -> Self {
        let n = groups.len();
        let hide_countdown = if url.is_empty() { 2 } else { 0 };
        Self {
            url, groups, config,
            expanded: None, selected: None,
            icons: vec![None; n], icons_loaded: false,
            drag_src: None, drag_tgt: 0,
            row_rects: vec![egui::Rect::NOTHING; n],
            detect,
            resident,
            incoming,
            hwnd,
            hide_countdown,
        }
    }

    fn save_order(&mut self) {
        self.config.browser_order = self.groups.iter().map(|g| g.exe_path.clone()).collect();
        let order = self.config.browser_order.clone();
        let _ = Config::update(|cfg| cfg.browser_order = order);
    }

    /// 選択・キャンセル後の片付け。常駐モードでは終了せず非表示にする
    fn dismiss(&self, ctx: &egui::Context) {
        match (self.resident, self.hwnd) {
            (true, Some(h)) => force_hide(h),
            _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
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
        let lang = crate::lang::get();

        if self.resident {
            // 別プロセスから受信した URL に差し替えて状態をリセット
            let new_url = self.incoming.lock().unwrap().take();
            if let Some(url) = new_url {
                self.url = url;
                self.expanded = None;
                self.selected = None;
                self.drag_src = None;
                self.hide_countdown = 0; // 表示が確定したので隠す予約を取り消す
                // 設定画面でルール等が変わっているかもしれないので読み直す
                if let Ok(cfg) = Config::load() {
                    self.config = cfg;
                }
                // ブラウザ構成の変化も拾う
                self.detect.spawn();
            }
            // ✕ボタンでは終了せず非表示にして常駐を続ける
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if let Some(h) = self.hwnd {
                    force_hide(h);
                }
            }
        }

        // スタートアップ起動: eframe による初回描画後の強制表示が済んでから隠す
        if self.hide_countdown > 0 && self.url.is_empty() {
            self.hide_countdown -= 1;
            if self.hide_countdown == 0 {
                if let Some(h) = self.hwnd {
                    force_hide(h);
                }
            } else {
                ctx.request_repaint();
            }
        }

        // バックグラウンド検出が完了していたら表示を更新（キャッシュ保存は take 内で行われる）
        if let Some(mut fresh) = self.detect.take() {
            self.config.sort_groups(&mut fresh);
            if browser::groups_differ(&fresh, &self.groups) {
                let n = fresh.len();
                self.groups = fresh;
                self.icons = vec![None; n];
                self.icons_loaded = false;
                self.row_rects = vec![egui::Rect::NOTHING; n];
            }
        }

        if !self.icons_loaded {
            self.icons_loaded = true;
            for (i, g) in self.groups.iter().enumerate() {
                if let Some(img) = crate::icon::load(&g.exe_path) {
                    self.icons[i] = Some(ctx.load_texture(&g.name, img, egui::TextureOptions::LINEAR));
                }
            }
        }

        if let Some(src) = self.drag_src {
            if let Some(py) = ctx.input(|i| i.pointer.hover_pos()).map(|p| p.y) {
                let mut tgt = self.row_rects.len();
                for (i, r) in self.row_rects.iter().enumerate() {
                    if py < r.center().y {
                        tgt = i;
                        break;
                    }
                }
                if tgt == src || tgt == src + 1 { tgt = src; }
                self.drag_tgt = tgt;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            // バイト位置で切ると多バイト文字の途中でパニックするため文字単位で切る
            let display_url = match self.url.char_indices().nth(50) {
                Some((idx, _)) => format!("{}...", &self.url[..idx]),
                None => self.url.clone(),
            };
            ui.label(egui::RichText::new(&display_url).weak().small());
            ui.add_space(12.0);
            ui.label(lang.which_browser);
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

                if is_dragging && self.drag_tgt == gi && self.drag_tgt != self.drag_src.unwrap_or(usize::MAX) {
                    let y = ui.cursor().min.y + 1.0;
                    draw_drop_line(ui, x_min, x_min + w, y);
                    ui.add_space(4.0);
                }

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

                if let Some(Some(tex)) = self.icons.get(gi) {
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.min.x + 10.0, rect.center().y - ICON_SIZE / 2.0),
                        egui::vec2(ICON_SIZE, ICON_SIZE),
                    );
                    ui.painter().image(tex.id(), icon_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }

                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                    &self.groups[gi].name, egui::FontId::proportional(14.0), visuals.text_color());

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

                if resp.drag_started() {
                    self.drag_src = Some(gi);
                    self.drag_tgt = gi;
                    self.expanded = None;
                }
                if resp.drag_stopped() {
                    if let Some(src) = self.drag_src.take() {
                        let tgt = self.drag_tgt;
                        if tgt != src && tgt != src + 1 {
                            let insert = if tgt > src { tgt - 1 } else { tgt };
                            let item = self.groups.remove(src);
                            self.groups.insert(insert, item);
                            let icon = self.icons.remove(src);
                            self.icons.insert(insert, icon);
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
                        self.dismiss(ctx);
                    }
                }

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
                            self.dismiss(ctx);
                        }
                    }
                }
            }

            let last = self.groups.len();
            if is_dragging && self.drag_tgt == last && self.drag_src != Some(last.saturating_sub(1)) {
                let y = ui.cursor().min.y + 1.0;
                draw_drop_line(ui, x_min, x_min + w, y);
                ui.add_space(4.0);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(lang.cancel).clicked() {
                    self.dismiss(ctx);
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("⚙ {}", lang.settings)).small()
                ).frame(false)).clicked() {
                    // 常駐したまま設定を開けるよう別プロセスで起動する
                    crate::util::spawn_self_detached(&[]);
                    self.dismiss(ctx);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match &self.config.update_available {
                        Some(tag) if crate::updater::is_newer(tag) => {
                            ui.label(egui::RichText::new(format!("⬆ {} {}", tag, lang.update_suffix))
                                .color(egui::Color32::from_rgb(80, 180, 80))
                                .small());
                        }
                        _ => {
                            ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .weak().small());
                        }
                    }
                });
            });
        });

        if let Some((gi, pi)) = self.selected {
            if let Some(b) = self.groups.get(gi).and_then(|g| g.browsers.get(pi)) {
                let _ = b.launch(&self.url);
            }
            self.selected = None;
        }
    }
}
