#[cfg(windows)]
#[path = "tray_windows.rs"]
mod platform;

#[cfg(windows)]
pub use platform::{cleanup, spawn};

#[cfg(target_os = "linux")]
pub fn spawn() {}

#[cfg(target_os = "linux")]
pub fn cleanup() {}
