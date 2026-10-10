#[cfg(windows)]
#[path = "icon_windows.rs"]
mod platform;

#[cfg(windows)]
pub use platform::load;

#[cfg(target_os = "linux")]
#[path = "icon_linux.rs"]
mod platform;

#[cfg(target_os = "linux")]
pub use platform::load;

pub fn load_browser(group: &crate::browser::BrowserGroup) -> Option<egui::ColorImage> {
    #[cfg(windows)]
    {
        load(&group.exe_path)
    }
    #[cfg(target_os = "linux")]
    {
        platform::load_browser(group)
    }
}
