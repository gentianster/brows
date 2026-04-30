use egui::ColorImage;
use std::mem;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, RGBQUAD,
};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};
use windows::core::PCWSTR;

const SIZE: i32 = 24;

pub fn load(exe_path: &str) -> Option<ColorImage> {
    unsafe { load_unsafe(exe_path) }
}

unsafe fn load_unsafe(exe_path: &str) -> Option<ColorImage> {
    // exe からアイコンハンドルを取得
    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut shfi: SHFILEINFOW = mem::zeroed();
    let result = SHGetFileInfoW(
        PCWSTR(wide.as_ptr()),
        Default::default(),
        Some(&mut shfi),
        mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_LARGEICON,
    );
    if result == 0 || shfi.hIcon.is_invalid() {
        return None;
    }
    let hicon = shfi.hIcon;

    // SIZE×SIZE の 32bit DIB セクションを作成
    let hdc = CreateCompatibleDC(None);
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0, // BI_RGB
            ..Default::default()
        },
        bmiColors: [RGBQUAD::default()],
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbm = match CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(h) => h,
        Err(_) => {
            let _ = DeleteDC(hdc);
            let _ = DestroyIcon(hicon);
            return None;
        }
    };

    let old = SelectObject(hdc, hbm);

    // ゼロクリアしてアイコンを描画
    let n = (SIZE * SIZE) as usize;
    std::ptr::write_bytes(bits as *mut u8, 0, n * 4);
    let _ = DrawIconEx(hdc, 0, 0, hicon, SIZE, SIZE, 0, None, DI_NORMAL);

    // BGRA → RGBA 変換
    let bgra = std::slice::from_raw_parts(bits as *const u8, n * 4);
    let mut rgba = vec![0u8; n * 4];
    for (i, b) in bgra.chunks(4).enumerate() {
        rgba[i * 4] = b[2];
        rgba[i * 4 + 1] = b[1];
        rgba[i * 4 + 2] = b[0];
        rgba[i * 4 + 3] = b[3];
    }
    // 古い形式のアイコンはアルファが 0 のため、非黒ピクセルを不透明にする
    if rgba.chunks(4).all(|c| c[3] == 0) {
        for c in rgba.chunks_mut(4) {
            if c[0] | c[1] | c[2] != 0 {
                c[3] = 255;
            }
        }
    }

    SelectObject(hdc, old);
    let _ = DeleteObject(hbm);
    let _ = DeleteDC(hdc);
    let _ = DestroyIcon(hicon);

    Some(ColorImage::from_rgba_unmultiplied([SIZE as usize, SIZE as usize], &rgba))
}
