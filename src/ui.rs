use crate::browser::{self, Browser, BrowserGroup};
use crate::config::{Config, Rule};
use crate::registry;
use crate::updater::{UpdateState, Updater};
use anyhow::Result;
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

fn app_icon() -> Option<Arc<egui::IconData>> {
    let exe = std::env::current_exe().ok()?;
    let img = crate::icon::load(&exe.to_string_lossy())?;
    let rgba = img.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), p.a()]).collect();
    Some(Arc::new(egui::IconData {
        rgba,
        width: img.width() as u32,
        height: img.height() as u32,
    }))
}

fn center_pos(win_w: f32, win_h: f32) -> egui::Pos2 {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f32;
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) } as f32;
    egui::pos2((sw - win_w) / 2.0, (sh - win_h) / 2.0)
}

static FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();

fn setup_fonts(cc: &eframe::CreationContext) {
    let data = FONT_DATA.get_or_init(|| {
        let candidates = [
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\msgothic.ttc",
        ];
        for path in &candidates {
            if let Ok(data) = std::fs::read(path) {
                return Some(data);
            }
        }
        None
    });
    let mut fonts = egui::FontDefinitions::default();
    if let Some(data) = data {
        fonts.font_data.insert("ja".to_owned(), egui::FontData::from_owned(data.clone()));
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().push("ja".to_owned());
    }
    cc.egui_ctx.set_fonts(fonts);
}

// ─── ブラウザ選択ピッカー ────────────────────────────────────────

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
    let mut config = Config::load()?;
    let has_cache = !config.cached_groups.is_empty();

    // キャッシュがあれば即使用、なければ初回のみ同期検出してキャッシュ保存
    let groups = if has_cache {
        let mut g = config.cached_groups.clone();
        if !config.browser_order.is_empty() {
            g.sort_by_key(|g| config.browser_order.iter().position(|o| o == &g.exe_path).unwrap_or(usize::MAX));
        }
        g
    } else {
        let mut g = browser::detect_grouped()?;
        if !config.browser_order.is_empty() {
            g.sort_by_key(|x| config.browser_order.iter().position(|o| o == &x.exe_path).unwrap_or(usize::MAX));
        }
        config.cached_groups = g.clone();
        let _ = config.save();
        g
    };

    // バックグラウンドで再検出してキャッシュを更新
    let pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>> = Arc::new(Mutex::new(None));
    let pending_clone = pending_groups.clone();
    std::thread::spawn(move || {
        if let Ok(fresh) = browser::detect_grouped() {
            *pending_clone.lock().unwrap() = Some(fresh);
        }
    });

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
    let incoming: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let incoming_clone = incoming.clone();

    eframe::run_native(
        "brows",
        options,
        Box::new(move |cc| {
            setup_fonts(cc);
            let hwnd = win32_hwnd(cc);
            if let Some(listener) = listener {
                spawn_ipc_server(listener, cc.egui_ctx.clone(), incoming_clone, hwnd);
            }
            Box::new(PickerApp::new(url.unwrap_or_default(), groups, config, pending_groups, resident, incoming, hwnd))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

fn win32_hwnd(cc: &eframe::CreationContext) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
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
                Some(crate::ipc::Request::Exit) => std::process::exit(0),
                None => {}
            }
        }
    });
}

/// 常駐ウィンドウを画面中央に再表示して前面に出す
fn force_show(hwnd: isize) {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, SetForegroundWindow, SetWindowPos, ShowWindow,
        HWND_TOPMOST, SM_CXSCREEN, SM_CYSCREEN, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOW,
    };
    let hwnd = HWND(hwnd);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
            let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let _ = SetWindowPos(
                hwnd, HWND_TOPMOST,
                (sw - w) / 2, (sh - h) / 2, 0, 0,
                SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

fn force_hide(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let _ = ShowWindow(HWND(hwnd), SW_HIDE);
    }
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
    pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>>,
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
        pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>>,
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
            pending_groups,
            resident,
            incoming,
            hwnd,
            hide_countdown,
        }
    }

    fn save_order(&mut self) {
        self.config.browser_order = self.groups.iter().map(|g| g.exe_path.clone()).collect();
        let _ = self.config.save();
    }

    /// 選択・キャンセル後の片付け。常駐モードでは終了せず非表示にする
    fn dismiss(&self, ctx: &egui::Context) {
        match (self.resident, self.hwnd) {
            (true, Some(h)) => force_hide(h),
            _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
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
                let pending = self.pending_groups.clone();
                std::thread::spawn(move || {
                    if let Ok(fresh) = browser::detect_grouped() {
                        *pending.lock().unwrap() = Some(fresh);
                    }
                });
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

        // バックグラウンド検出が完了していたらキャッシュ更新
        if let Ok(mut lock) = self.pending_groups.try_lock() {
            if let Some(fresh) = lock.take() {
                let mut sorted = fresh.clone();
                if !self.config.browser_order.is_empty() {
                    sorted.sort_by_key(|g| self.config.browser_order.iter().position(|o| o == &g.exe_path).unwrap_or(usize::MAX));
                }
                let changed = sorted.len() != self.groups.len()
                    || sorted.iter().zip(&self.groups).any(|(a, b)| a.exe_path != b.exe_path);
                if changed {
                    let n = sorted.len();
                    self.groups = sorted;
                    self.icons = vec![None; n];
                    self.icons_loaded = false;
                    self.row_rects = vec![egui::Rect::NOTHING; n];
                }
                std::thread::spawn(move || {
                    if let Ok(mut cfg) = crate::config::Config::load() {
                        cfg.cached_groups = fresh;
                        let _ = cfg.save();
                    }
                });
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
                            let item = self.groups.remove(src);
                            let insert = if tgt > src { tgt - 1 } else { tgt };
                            self.groups.insert(insert, item);
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
                    crate::relaunch_settings();
                    self.dismiss(ctx);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(tag) = &self.config.update_available {
                        if crate::updater::is_newer(tag) {
                            ui.label(egui::RichText::new(format!("⬆ {} {}", tag, lang.update_suffix))
                                .color(egui::Color32::from_rgb(80, 180, 80))
                                .small());
                        } else {
                            ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .weak().small());
                        }
                    } else {
                        ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .weak().small());
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

// ─── 設定画面 ────────────────────────────────────────────────────

pub fn show_settings() -> Result<()> {
    let lang = crate::lang::get();
    let mut config = Config::load().unwrap_or_default();

    let groups = if !config.cached_groups.is_empty() {
        config.cached_groups.clone()
    } else {
        let g = browser::detect_grouped().unwrap_or_default();
        config.cached_groups = g.clone();
        let _ = config.save();
        g
    };

    let pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>> = Arc::new(Mutex::new(None));
    let pending_clone = pending_groups.clone();
    std::thread::spawn(move || {
        if let Ok(fresh) = browser::detect_grouped() {
            *pending_clone.lock().unwrap() = Some(fresh);
        }
    });

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
            Box::new(SettingsApp::new(groups, config, pending_groups))
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
    pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>>,
}

impl SettingsApp {
    fn new(groups: Vec<BrowserGroup>, config: Config, pending_groups: Arc<Mutex<Option<Vec<BrowserGroup>>>>) -> Self {
        let registered = is_registered();
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
            pending_groups,
        }
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

/// 常駐ピッカーを別プロセスで起動する（既に常駐がいれば即終了するので無害）
fn spawn_resident() {
    use std::os::windows::process::CommandExt;
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg("--resident")
            .creation_flags(0x00000008) // DETACHED_PROCESS
            .spawn();
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
        let lang = crate::lang::get();

        if let Ok(mut lock) = self.pending_groups.try_lock() {
            if let Some(fresh) = lock.take() {
                let changed = fresh.len() != self.groups.len()
                    || fresh.iter().zip(&self.groups).any(|(a, b)| a.exe_path != b.exe_path);
                if changed {
                    self.groups = fresh.clone();
                    self.icons.clear();
                    self.icons_loaded = false;
                }
                std::thread::spawn(move || {
                    if let Ok(mut cfg) = crate::config::Config::load() {
                        cfg.cached_groups = fresh;
                        let _ = cfg.save();
                    }
                });
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
                            ui.label(egui::RichText::new(lang.up_to_date).weak().small());
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
                            spawn_resident();
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
                        .creation_flags(0x08000000)
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
                let _ = self.config.save();
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
                    let _ = self.config.save();
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
