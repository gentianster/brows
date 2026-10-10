use crate::browser::BrowserGroup;
use egui::ColorImage;
use gio::prelude::*;
use gio::{DesktopAppInfo, FileIcon, Icon, ThemedIcon};
use image::{io::Limits, io::Reader, ImageFormat};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

const MAX_PNG_BYTES: u64 = 8 * 1024 * 1024;
const SIZES: [u16; 9] = [48, 64, 32, 128, 256, 24, 22, 16, 512];
const CONTEXTS: [&str; 8] = [
    "apps",
    "actions",
    "categories",
    "devices",
    "emblems",
    "mimetypes",
    "places",
    "status",
];

struct SearchPaths {
    icons: Vec<PathBuf>,
    pixmaps: Vec<PathBuf>,
}

impl SearchPaths {
    fn from_env() -> Self {
        let home = dirs_next::home_dir();
        let data_home = absolute_env_path("XDG_DATA_HOME")
            .or_else(|| home.as_ref().map(|path| path.join(".local/share")));
        let data_dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .map(|value| {
                std::env::split_paths(&value)
                    .filter(|path| path.is_absolute())
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
            });
        let mut icons = Vec::new();
        if let Some(path) = home {
            icons.push(path.join(".icons"));
        }
        let data_roots = data_home.into_iter().chain(data_dirs);
        let mut pixmaps = Vec::new();
        for path in data_roots {
            icons.push(path.join("icons"));
            pixmaps.push(path.join("pixmaps"));
        }
        Self { icons, pixmaps }
    }
}

fn absolute_env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

pub fn load(path: &str) -> Option<ColorImage> {
    load_png(Path::new(path))
}

pub fn load_browser(group: &BrowserGroup) -> Option<ColorImage> {
    load_browser_with_paths(group, &SearchPaths::from_env())
}

fn load_browser_with_paths(group: &BrowserGroup, paths: &SearchPaths) -> Option<ColorImage> {
    let app = group
        .desktop_id
        .as_deref()
        .and_then(DesktopAppInfo::new)
        .or_else(|| {
            group.browsers.iter().find_map(|browser| {
                browser
                    .desktop_file
                    .as_deref()
                    .and_then(DesktopAppInfo::from_filename)
            })
        })?;
    load_icon(&app.icon()?, paths)
}

fn load_icon(icon: &Icon, paths: &SearchPaths) -> Option<ColorImage> {
    if let Some(file) = icon.downcast_ref::<FileIcon>() {
        return load_png(&file.file().path()?);
    }
    let themed = icon.downcast_ref::<ThemedIcon>()?;
    for name in themed.names() {
        if !matches!(
            Path::new(name.as_str())
                .components()
                .collect::<Vec<_>>()
                .as_slice(),
            [Component::Normal(_)]
        ) {
            continue;
        }
        let filename = if name.ends_with(".png") {
            name.to_string()
        } else {
            format!("{name}.png")
        };
        for root in &paths.icons {
            for size in SIZES {
                for context in CONTEXTS {
                    let candidate = root
                        .join("hicolor")
                        .join(format!("{size}x{size}"))
                        .join(context)
                        .join(&filename);
                    if let Some(image) = load_png(&candidate) {
                        return Some(image);
                    }
                }
            }
            if let Some(image) = load_png(&root.join(&filename)) {
                return Some(image);
            }
        }
        for root in &paths.pixmaps {
            if let Some(image) = load_png(&root.join(&filename)) {
                return Some(image);
            }
        }
    }
    // SVG decoding and active-theme inheritance would require another rendering stack.
    None
}

fn load_png(path: &Path) -> Option<ColorImage> {
    if !path.is_absolute() || !path.extension()?.eq_ignore_ascii_case("png") {
        return None;
    }
    // A FIFO icon path must not stall the picker while waiting for a writer.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PNG_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_PNG_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_PNG_BYTES {
        return None;
    }
    let mut reader = Reader::with_format(std::io::Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.into_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::Browser;
    use std::os::unix::ffi::OsStrExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn write_png(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(2, 3, image::Rgba([13, 28, 99, 255]))
            .save(path)
            .unwrap();
    }

    fn paths(root: &Path) -> SearchPaths {
        SearchPaths {
            icons: vec![root.join("icons"), root.join("legacy-icons")],
            pixmaps: vec![root.join("pixmaps")],
        }
    }

    #[test]
    fn absolute_desktop_entry_icon_preserves_pixels_and_dimensions() {
        let directory = tempfile::tempdir().unwrap();
        let png = directory.path().join("日本語 icon.png");
        write_png(&png);
        let desktop = directory.path().join("fixture.desktop");
        std::fs::write(
            &desktop,
            format!(
                "[Desktop Entry]\nType=Application\nName=Fixture\nExec=/bin/true %u\nIcon={}\n",
                png.display()
            ),
        )
        .unwrap();
        let group = BrowserGroup {
            name: "Fixture".into(),
            exe_path: "/bin/true".into(),
            desktop_id: None,
            browsers: vec![Browser {
                name: "Fixture".into(),
                exe_path: "/bin/true".into(),
                profile_dir: None,
                desktop_id: None,
                desktop_file: Some(desktop.to_str().unwrap().into()),
                desktop_exec: None,
            }],
        };
        let image = load_browser_with_paths(&group, &paths(directory.path())).unwrap();
        assert_eq!(image.size, [2, 3]);
        assert!(image
            .pixels
            .iter()
            .all(|pixel| pixel.to_array() == [13, 28, 99, 255]));
        let link = directory.path().join("symlink.png");
        std::os::unix::fs::symlink(&png, &link).unwrap();
        assert_eq!(load_png(&link).unwrap().size, image.size);
    }

    #[test]
    fn themed_names_fall_back_past_missing_or_corrupt_hicolor_pngs() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(directory.path());
        let corrupt = paths.icons[0].join("hicolor/48x48/apps/fixture.png");
        std::fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
        std::fs::write(corrupt, b"not a PNG").unwrap();
        write_png(&paths.icons[0].join("hicolor/32x32/apps/fixture.png"));
        let icon: Icon = ThemedIcon::from_names(&["missing", "fixture"]).upcast();
        assert_eq!(load_icon(&icon, &paths).unwrap().size, [2, 3]);
    }

    #[test]
    fn legacy_icons_and_pixmaps_are_png_fallbacks() {
        let directory = tempfile::tempdir().unwrap();
        let paths = paths(directory.path());
        write_png(&paths.icons[1].join("legacy.png"));
        write_png(&paths.pixmaps[0].join("pixmap.png"));
        for name in ["legacy", "pixmap"] {
            let icon: Icon = ThemedIcon::new(name).upcast();
            assert!(load_icon(&icon, &paths).is_some());
        }
    }

    #[test]
    fn missing_corrupt_relative_svg_and_traversal_icons_are_ignored() {
        let directory = tempfile::tempdir().unwrap();
        let corrupt = directory.path().join("corrupt.png");
        std::fs::write(&corrupt, b"not a PNG").unwrap();
        for path in [
            directory.path().join("missing.png"),
            corrupt,
            PathBuf::from("relative.png"),
            directory.path().join("icon.svg"),
        ] {
            assert!(load_png(&path).is_none());
        }
        let icon: Icon = ThemedIcon::new("../outside").upcast();
        assert!(load_icon(&icon, &paths(directory.path())).is_none());
    }

    #[test]
    fn named_fifo_icon_does_not_wait_for_a_writer() {
        const WORKER_PATH: &str = "BROWS_ICON_FIFO_TEST_PATH";
        if let Some(path) = std::env::var_os(WORKER_PATH) {
            assert!(load_png(Path::new(&path)).is_none());
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("named-fifo.png");
        let filename = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(filename.as_ptr(), 0o600) }, 0);
        // A blocking-open regression must fail within a bound instead of hanging cargo test.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "icon::platform::tests::named_fifo_icon_does_not_wait_for_a_writer",
            ])
            .env(WORKER_PATH, &path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "FIFO icon worker failed: {status}");
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("FIFO icon lookup waited for a writer");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
