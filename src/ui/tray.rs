//! タスクトレイ（通知領域）アイコン。常駐中に設定画面を開く導線を提供する。
//!
//! eframe（winit）のイベントループとは独立した専用スレッドで非表示ウィンドウと
//! メッセージループを持ち、Win32 の `Shell_NotifyIconW` でアイコンを登録する。
//! 左クリックで設定画面を開き、右クリックで「設定を開く／常駐トグル」メニューを出す。

use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    ExtractIconW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, LoadIconW, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu, TranslateMessage, HICON,
    IDI_APPLICATION, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPED,
};

/// トレイアイコンからのマウス通知を受け取るコールバックメッセージ
const WM_TRAY: u32 = WM_APP + 1;
/// 通知領域内でアイコンを識別する ID（1 プロセス 1 アイコン）
const ICON_UID: u32 = 1;
/// 右クリックメニューのコマンド ID
const ID_SETTINGS: usize = 1;
const ID_STARTUP: usize = 2;

/// 終了処理でアイコンを除去できるよう、トレイウィンドウの HWND を保持する
static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);

/// 専用スレッドでトレイアイコンとメッセージループを開始する
pub fn spawn() {
    std::thread::spawn(|| unsafe { run() });
}

/// プロセス終了前にトレイアイコンを除去する（ゴーストアイコン防止）
pub fn cleanup() {
    let h = TRAY_HWND.load(Ordering::SeqCst);
    if h != 0 {
        unsafe { remove_icon(HWND(h)) };
    }
}

/// Explorer が再起動したときにアイコンを再登録するためのブロードキャストメッセージ
fn taskbar_created() -> u32 {
    static MSG_ID: OnceLock<u32> = OnceLock::new();
    *MSG_ID.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
}

fn module_handle() -> HINSTANCE {
    let h = unsafe { GetModuleHandleW(None) }.map(|m| m.0).unwrap_or(0);
    HINSTANCE(h)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn run() {
    let hinst = module_handle();
    let class_name = w!("brows_tray_window");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinst,
        lpszClassName: class_name,
        ..Default::default()
    };
    RegisterClassW(&wc);

    // 表示しないトップレベルウィンドウ。ShowWindow を呼ばないのでタスクバーにも
    // 出ないが、TaskbarCreated のブロードキャストは受け取れる（message-only window
    // だと受け取れないためトップレベルにしている）。
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!("brows"),
        WS_OVERLAPPED,
        0, 0, 0, 0,
        None, None, hinst, None,
    );
    if hwnd.0 == 0 {
        return;
    }

    TRAY_HWND.store(hwnd.0, Ordering::SeqCst);
    add_icon(hwnd);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_TRAY {
        // 非 GUID 登録ではマウスメッセージが lParam の下位ワードに入る
        match (lparam.0 as u32) & 0xFFFF {
            WM_LBUTTONUP => open_settings(),
            WM_RBUTTONUP => show_menu(hwnd),
            _ => {}
        }
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        remove_icon(hwnd);
        PostQuitMessage(0);
        return LRESULT(0);
    }
    if msg == taskbar_created() {
        add_icon(hwnd);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 右クリックメニュー（設定を開く・常駐トグル）を表示してコマンドを処理する
unsafe fn show_menu(hwnd: HWND) {
    let lang = crate::lang::get();
    let Ok(menu) = CreatePopupMenu() else { return };

    let settings = to_wide(lang.tray_settings);
    let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, PCWSTR(settings.as_ptr()));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let startup = to_wide(lang.startup_checkbox);
    let check = if crate::registry::is_startup_registered() { MF_CHECKED } else { MF_UNCHECKED };
    let _ = AppendMenuW(menu, MF_STRING | check, ID_STARTUP, PCWSTR(startup.as_ptr()));

    // メニュー外クリックでも確実に閉じるよう、表示前後に前面化とダミー送信を行う
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, pt.x, pt.y, 0, hwnd, None);
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(menu);

    match cmd.0 as usize {
        ID_SETTINGS => open_settings(),
        ID_STARTUP => toggle_startup(),
        _ => {}
    }
}

/// 設定画面を別プロセスで開く（常駐インスタンスはそのまま動かし続ける）
fn open_settings() {
    crate::util::spawn_self_detached(&[]);
}

/// 「Windows 起動時に常駐する」のオン/オフを切り替える
fn toggle_startup() {
    if crate::registry::is_startup_registered() {
        let _ = crate::registry::unregister_startup();
    } else {
        let _ = crate::registry::register_startup();
    }
}

unsafe fn add_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_UID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: load_app_icon(),
        ..Default::default()
    };
    let tip: Vec<u16> = "brows".encode_utf16().collect();
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_UID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

/// exe に埋め込まれたアイコンを取り出す。取れなければ既定のアプリアイコン
unsafe fn load_app_icon() -> HICON {
    if let Ok(exe) = std::env::current_exe() {
        let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let icon = ExtractIconW(module_handle(), PCWSTR(wide.as_ptr()), 0);
        // 0 = アイコンなし / 1 = 実行ファイルでない、のいずれでもなければ有効
        if icon.0 != 0 && icon.0 != 1 {
            return icon;
        }
    }
    LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
}
