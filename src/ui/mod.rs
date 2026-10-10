mod picker;
mod settings;
mod tray;
#[cfg(windows)]
mod win32;
mod window;
#[cfg(target_os = "linux")]
mod x11;

pub use picker::{open_url, run_resident};
pub use settings::show_settings;

use eframe::egui;
use std::sync::{Arc, OnceLock};

fn app_icon() -> Option<Arc<egui::IconData>> {
    let exe = std::env::current_exe().ok()?;
    let img = crate::icon::load(&exe.to_string_lossy())?;
    let rgba = img
        .pixels
        .iter()
        .flat_map(|p| [p.r(), p.g(), p.b(), p.a()])
        .collect();
    Some(Arc::new(egui::IconData {
        rgba,
        width: img.width() as u32,
        height: img.height() as u32,
    }))
}

static FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();

fn native_options(viewport: egui::ViewportBuilder, centered: bool) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport,
        centered,
        #[cfg(target_os = "linux")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            // glutin-winit also enables Wayland, even when eframe only requests X11.
            builder.with_x11();
        })),
        ..Default::default()
    }
}

fn setup_fonts(cc: &eframe::CreationContext) {
    let data = FONT_DATA.get_or_init(|| {
        #[cfg(windows)]
        let candidates = [
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\msgothic.ttc",
        ];
        #[cfg(target_os = "linux")]
        let candidates = [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
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
        fonts
            .font_data
            .insert("ja".to_owned(), egui::FontData::from_owned(data.clone()));
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .push("ja".to_owned());
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("ja".to_owned());
    }
    cc.egui_ctx.set_fonts(fonts);
}
