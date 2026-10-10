#[cfg(windows)]
#[path = "updater_windows.rs"]
mod platform;

#[cfg(windows)]
pub use platform::*;

#[cfg(target_os = "linux")]
pub fn check_if_due() {}

#[cfg(target_os = "linux")]
pub fn is_newer(_tag: &str) -> bool {
    // Linux releases have no installer or replacement contract yet.
    false
}
