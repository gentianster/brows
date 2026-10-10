#[cfg(windows)]
pub type NativeWindow = isize;
#[cfg(target_os = "linux")]
pub type NativeWindow = u32;

#[cfg(windows)]
pub use super::win32::{center_pos, force_hide, force_show, hwnd_of as window_of};
#[cfg(target_os = "linux")]
pub use super::x11::{center_pos, force_hide, force_show, window_of};
