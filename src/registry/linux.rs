use anyhow::{anyhow, bail, Context, Result};
use gio::glib::{KeyFile, KeyFileFlags};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const DESKTOP_ID: &str = "brows.desktop";
const GROUP: &str = "Desktop Entry";
const MANAGED_KEY: &str = "X-Brows-Managed";
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct RegistrationPaths {
    applications: PathBuf,
    autostart: PathBuf,
}

impl RegistrationPaths {
    fn for_user() -> Result<Self> {
        let home = dirs_next::home_dir().context("Cannot find the user's home directory")?;
        let data = xdg_home("XDG_DATA_HOME", home.join(".local/share"));
        let config = xdg_home("XDG_CONFIG_HOME", home.join(".config"));
        Ok(Self::new(data, config))
    }

    fn new(data: PathBuf, config: PathBuf) -> Self {
        Self {
            applications: data.join("applications"),
            autostart: config.join("autostart"),
        }
    }

    fn application_file(&self) -> PathBuf {
        self.applications.join(DESKTOP_ID)
    }

    fn autostart_file(&self) -> PathBuf {
        self.autostart.join(DESKTOP_ID)
    }
}

fn xdg_home(variable: &str, fallback: PathBuf) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or(fallback)
}

fn exec_argument(executable: &Path) -> Result<String> {
    let value = executable
        .to_str()
        .context("The executable path must be UTF-8")?;
    if !executable.is_absolute() || value.contains('=') || value.chars().any(char::is_control) {
        bail!("The executable path must be absolute and contain no '=' or control characters");
    }
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' | '"' | '$' | '`' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    Ok(quoted)
}

fn desktop_entry(executable: &Path, startup: bool) -> Result<String> {
    let argument = exec_argument(executable)?;
    // GIO checks Exec's executable before expanding %% into a literal percent.
    let prefix = if executable.as_os_str().as_encoded_bytes().contains(&b'%') {
        "/usr/bin/env -- "
    } else {
        ""
    };
    let suffix = if startup { " --resident" } else { " %u" };
    let key = KeyFile::new();
    key.set_string(GROUP, "Type", "Application");
    key.set_string(GROUP, "Name", "brows");
    key.set_string(GROUP, "Comment", "Choose a browser for each link");
    key.set_string(GROUP, "Exec", &format!("{prefix}{argument}{suffix}"));
    key.set_string(GROUP, "Icon", "web-browser");
    key.set_boolean(GROUP, "Terminal", false);
    key.set_boolean(GROUP, MANAGED_KEY, true);
    if startup {
        key.set_boolean(GROUP, "NoDisplay", true);
    } else {
        key.set_string(GROUP, "Categories", "Network;WebBrowser;");
        key.set_string(
            GROUP,
            "MimeType",
            "x-scheme-handler/http;x-scheme-handler/https;",
        );
    }
    Ok(key.to_data().to_string())
}

fn write_entry(path: &Path, executable: &Path, startup: bool) -> Result<()> {
    let data = desktop_entry(executable, startup)?;
    if path.exists() && !is_managed(path) {
        bail!(
            "Refusing to replace an unrelated Desktop Entry: {}",
            path.display()
        );
    }
    let parent = path
        .parent()
        .context("The Desktop Entry has no parent directory")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".brows.{}.{}.tmp",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temp)?;
        file.write_all(data.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.with_context(|| format!("Cannot write {}", path.display()))
}

fn is_managed(path: &Path) -> bool {
    let key = KeyFile::new();
    key.load_from_file(path, KeyFileFlags::NONE).is_ok()
        && key.boolean(GROUP, MANAGED_KEY).unwrap_or(false)
        && key
            .string(GROUP, "Type")
            .map(|value| value == "Application")
            .unwrap_or(false)
}

fn remove_entry(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if !is_managed(path) {
        bail!(
            "Refusing to remove an unrelated Desktop Entry: {}",
            path.display()
        );
    }
    fs::remove_file(path).with_context(|| format!("Cannot remove {}", path.display()))
}

fn refresh_cache(applications: &Path) -> Result<()> {
    let output = Command::new("update-desktop-database")
        .arg(applications)
        .output()
        .context("Cannot update the Desktop Entry cache; install desktop-file-utils")?;
    if !output.status.success() {
        return Err(anyhow!(
            "Cannot update the Desktop Entry cache: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub fn register() -> Result<()> {
    let paths = RegistrationPaths::for_user()?;
    write_entry(&paths.application_file(), &std::env::current_exe()?, false)?;
    refresh_cache(&paths.applications)
}

pub fn unregister() -> Result<()> {
    let paths = RegistrationPaths::for_user()?;
    remove_entry(&paths.application_file())?;
    if paths.applications.exists() {
        refresh_cache(&paths.applications)?;
    }
    Ok(())
}

pub fn is_registered() -> bool {
    RegistrationPaths::for_user()
        .map(|paths| is_managed(&paths.application_file()))
        .unwrap_or(false)
}

pub fn register_startup() -> Result<()> {
    let paths = RegistrationPaths::for_user()?;
    write_entry(&paths.autostart_file(), &std::env::current_exe()?, true)
}

pub fn unregister_startup() -> Result<()> {
    remove_entry(&RegistrationPaths::for_user()?.autostart_file())
}

pub fn is_startup_registered() -> bool {
    RegistrationPaths::for_user()
        .map(|paths| {
            let path = paths.autostart_file();
            let key = KeyFile::new();
            is_managed(&path)
                && key.load_from_file(path, KeyFileFlags::NONE).is_ok()
                && !key.boolean(GROUP, "Hidden").unwrap_or(false)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gio::prelude::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::time::{Duration, Instant};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "brows-registration-test-{}-{}",
                std::process::id(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn desktop_entry_passes_special_path_and_url_as_single_arguments() {
        let fixture = Fixture::new();
        let executable = fixture.0.join("brows space \"日本語%\\$`");
        let result = fixture.0.join("received.txt");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$0\" \"$@\" > \"$BROWS_TEST_OUTPUT\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let desktop = fixture.0.join(DESKTOP_ID);
        write_entry(&desktop, &executable, false).unwrap();
        let app = gio::DesktopAppInfo::from_filename(&desktop).unwrap();
        assert!(app.supports_uris());
        assert!(app
            .supported_types()
            .iter()
            .any(|value| value == "x-scheme-handler/http"));
        assert!(app
            .supported_types()
            .iter()
            .any(|value| value == "x-scheme-handler/https"));
        let context = gio::AppLaunchContext::new();
        context.setenv("BROWS_TEST_OUTPUT", result.to_str().unwrap());
        let uri = "https://example.invalid/a%20b?x=日本語&y=\"value\"";
        app.launch_uris(&[uri], Some(&context)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let expected = format!("{}\n{uri}\n", executable.display());
        loop {
            if fs::read_to_string(&result).ok().as_deref() == Some(expected.as_str()) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "The synthetic launcher did not receive the expected arguments"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn autostart_uses_the_stable_executable_and_resident_argument() {
        let key = KeyFile::new();
        key.load_from_data(
            &desktop_entry(Path::new("/opt/brows tools/brows"), true).unwrap(),
            KeyFileFlags::NONE,
        )
        .unwrap();
        assert_eq!(
            key.string(GROUP, "Exec").unwrap(),
            "\"/opt/brows tools/brows\" --resident"
        );
        assert!(key.boolean(GROUP, "NoDisplay").unwrap());
        assert!(key.string(GROUP, "MimeType").is_err());
    }

    #[test]
    fn registration_and_removal_only_touch_the_isolated_desktop_entries() {
        let fixture = Fixture::new();
        let paths = RegistrationPaths::new(fixture.0.join("data"), fixture.0.join("config"));
        fs::create_dir_all(fixture.0.join("config")).unwrap();
        let defaults = fixture.0.join("config/mimeapps.list");
        fs::write(
            &defaults,
            "[Default Applications]\nx-scheme-handler/http=other.desktop;\n",
        )
        .unwrap();
        let before = fs::read(&defaults).unwrap();
        for (path, startup) in [
            (paths.application_file(), false),
            (paths.autostart_file(), true),
        ] {
            write_entry(&path, Path::new("/opt/brows"), startup).unwrap();
            assert!(is_managed(&path));
            if !startup {
                refresh_cache(&paths.applications).unwrap();
                let cache = fs::read_to_string(paths.applications.join("mimeinfo.cache")).unwrap();
                assert!(cache.contains("x-scheme-handler/http=brows.desktop;"));
                assert!(cache.contains("x-scheme-handler/https=brows.desktop;"));
            }
            remove_entry(&path).unwrap();
            remove_entry(&path).unwrap();
            assert!(!path.exists());
        }
        assert_eq!(fs::read(defaults).unwrap(), before);
    }

    #[test]
    fn unrelated_desktop_entry_is_preserved() {
        let fixture = Fixture::new();
        let desktop = fixture.0.join(DESKTOP_ID);
        let original =
            "[Desktop Entry]\nType=Application\nName=Other application\nExec=/bin/true\n";
        fs::write(&desktop, original).unwrap();
        assert!(write_entry(&desktop, Path::new("/opt/brows"), false).is_err());
        assert!(remove_entry(&desktop).is_err());
        assert_eq!(fs::read_to_string(desktop).unwrap(), original);
    }

    #[test]
    fn unsafe_or_unrepresentable_executable_paths_are_rejected() {
        for path in [
            "relative/brows",
            "/tmp/brows=other",
            "/tmp/brows\nHidden=true",
            "/tmp/brows\t",
        ] {
            assert!(desktop_entry(Path::new(path), false).is_err(), "{path:?}");
        }
    }
}
