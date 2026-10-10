use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{Mutex, OnceLock};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, StackMode,
};
use x11rb::rust_connection::RustConnection;

use super::window::NativeWindow;

type ConnectionState = Mutex<(RustConnection, usize)>;

fn with_connection<T>(f: impl FnOnce(&RustConnection, usize) -> Option<T>) -> Option<T> {
    static CONNECTION: OnceLock<Option<ConnectionState>> = OnceLock::new();
    // A separate connection avoids using winit's borrowed Xlib Display from the IPC thread.
    let connection = CONNECTION.get_or_init(|| x11rb::connect(None).ok().map(Mutex::new));
    let guard = connection.as_ref()?.lock().ok()?;
    f(&guard.0, guard.1)
}

pub fn center_pos(win_w: f32, win_h: f32) -> egui::Pos2 {
    with_connection(|conn, screen| {
        let screen = conn.setup().roots.get(screen)?;
        Some(egui::pos2(
            (f32::from(screen.width_in_pixels) - win_w) / 2.0,
            (f32::from(screen.height_in_pixels) - win_h) / 2.0,
        ))
    })
    .unwrap_or_else(|| egui::pos2(0.0, 0.0))
}

pub fn window_of(cc: &eframe::CreationContext) -> Option<NativeWindow> {
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window.try_into().ok().filter(|id| *id != 0),
        RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
        _ => None,
    }
}

pub fn force_show(window: NativeWindow) {
    let _ = with_connection(|conn, screen| {
        let screen = conn.setup().roots.get(screen)?;
        let geometry = conn.get_geometry(window).ok()?.reply().ok()?;
        let x = (i32::from(screen.width_in_pixels) - i32::from(geometry.width)) / 2;
        let y = (i32::from(screen.height_in_pixels) - i32::from(geometry.height)) / 2;
        conn.configure_window(
            window,
            &ConfigureWindowAux::new()
                .x(x)
                .y(y)
                .stack_mode(StackMode::ABOVE),
        )
        .ok()?
        .check()
        .ok()?;
        conn.map_window(window).ok()?.check().ok()?;
        let active = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let event = ClientMessageEvent::new(32, window, active, [1, x11rb::CURRENT_TIME, 0, 0, 0]);
        conn.send_event(
            false,
            screen.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .ok()?;
        conn.flush().ok()
    });
}

pub fn force_hide(window: NativeWindow) {
    let _ = with_connection(|conn, _| {
        conn.unmap_window(window).ok()?.check().ok()?;
        conn.flush().ok()
    });
}
