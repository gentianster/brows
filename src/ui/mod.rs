mod picker;
mod settings;
mod win32;

pub use picker::{open_url, run_resident};
pub use settings::show_settings;

use eframe::egui;
use std::sync::{Arc, OnceLock};

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
