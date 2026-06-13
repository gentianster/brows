//! egui からは扱えないウィンドウ操作を Win32 API で直接行うヘルパー

use eframe::egui;

/// プライマリモニタ中央に配置するためのウィンドウ位置を返す
pub fn center_pos(win_w: f32, win_h: f32) -> egui::Pos2 {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f32;
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) } as f32;
    egui::pos2((sw - win_w) / 2.0, (sh - win_h) / 2.0)
}

/// eframe ウィンドウの HWND を取り出す
pub fn hwnd_of(cc: &eframe::CreationContext) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// 常駐ウィンドウを画面中央に再表示して前面に出す
pub fn force_show(hwnd: isize) {
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

pub fn force_hide(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let _ = ShowWindow(HWND(hwnd), SW_HIDE);
    }
}
