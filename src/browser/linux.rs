use super::Browser;
use anyhow::{anyhow, bail, Context, Result};
use gio::glib::{KeyFile, KeyFileFlags};
use gio::prelude::*;
use gio::{AppInfo, AppLaunchContext, DesktopAppInfo};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DESKTOP_GROUP: &str = "Desktop Entry";

pub(super) fn detect() -> Result<Vec<Browser>> {
    let self_exe = std::env::current_exe().ok();
    let mut browsers = BTreeMap::new();
    for mime_type in ["x-scheme-handler/http", "x-scheme-handler/https"] {
        for app in AppInfo::all_for_type(mime_type) {
            let Ok(app) = app.downcast::<DesktopAppInfo>() else {
                continue;
            };
            let Some(id) = app.id() else {
                continue;
            };
            if let Some(browser) = browser_from_app(&app, id.as_str(), self_exe.as_deref()) {
                browsers.insert(id.to_string(), browser);
            }
        }
    }
    Ok(browsers.into_values().collect())
}

fn browser_from_app(app: &DesktopAppInfo, id: &str, self_exe: Option<&Path>) -> Option<Browser> {
    let executable = executable_path(app);
    if !app.supports_uris()
        || app.is_hidden()
        || !app.should_show()
        || is_self(id, &executable, self_exe)
    {
        return None;
    }
    Some(Browser {
        name: app.display_name().to_string(),
        exe_path: executable.to_string_lossy().into_owned(),
        profile_dir: None,
        desktop_id: Some(id.to_string()),
        desktop_file: app
            .filename()
            .map(|path| path.to_string_lossy().into_owned()),
        desktop_exec: app.string("Exec").map(|exec| exec.to_string()),
    })
}

fn executable_path(app: &DesktopAppInfo) -> PathBuf {
    // GIO's executable metadata splits at spaces and retains quotes around paths.
    app.commandline()
        .and_then(|line| gio::glib::shell_parse_argv(line).ok())
        .and_then(|arguments| arguments.into_iter().next())
        .map(PathBuf::from)
        .unwrap_or_else(|| app.executable())
}

fn is_self(id: &str, executable: &Path, self_exe: Option<&Path>) -> bool {
    if matches!(id, "brows.desktop" | "org.gentianster.brows.desktop") {
        return true;
    }
    self_exe.is_some_and(|current| {
        let executable =
            gio::glib::find_program_in_path(executable).unwrap_or_else(|| executable.to_path_buf());
        executable == current
            || executable
                .canonicalize()
                .ok()
                .zip(current.canonicalize().ok())
                .is_some_and(|(app, current)| app == current)
    })
}

fn app_for_browser(browser: &Browser) -> Result<DesktopAppInfo> {
    browser
        .desktop_id
        .as_deref()
        .and_then(DesktopAppInfo::new)
        .or_else(|| {
            browser
                .desktop_file
                .as_deref()
                .and_then(DesktopAppInfo::from_filename)
        })
        .ok_or_else(|| anyhow!("Desktop Entry is unavailable for {}", browser.name))
}

pub(super) fn launch(browser: &Browser, url: &str) -> Result<()> {
    if !crate::ipc::is_web_url(url) {
        bail!("Only HTTP and HTTPS URLs can be opened");
    }
    let original = app_for_browser(browser)?;
    if browser.profile_dir.is_some() && native_browser(browser, &original).is_none() {
        bail!(
            "Profile selection is supported only for native Vivaldi, Chrome and Chromium entries"
        );
    }
    launch_app(&original, browser.profile_dir.as_deref(), url)
}

fn launch_app(original: &DesktopAppInfo, profile: Option<&str>, url: &str) -> Result<()> {
    let app = match profile {
        Some(profile) => profile_app(original, profile)?,
        None => original.clone(),
    };
    app.launch_uris(&[url], None::<&AppLaunchContext>)?;
    Ok(())
}

fn profile_app(app: &DesktopAppInfo, profile_dir: &str) -> Result<DesktopAppInfo> {
    let filename = app.filename().context("Desktop Entry has no filename")?;
    let key_file = KeyFile::new();
    key_file.load_from_file(filename, KeyFileFlags::KEEP_TRANSLATIONS)?;
    let exec = key_file.string(DESKTOP_GROUP, "Exec")?;
    if !can_add_profile(&exec) {
        bail!("Desktop Entry requires its own profile or data directory");
    }
    if profile_dir != "Default" && !profile_dir.starts_with("Profile ") {
        bail!("Unsupported Chromium profile directory");
    }
    if Path::new(profile_dir).components().count() != 1 || profile_dir.chars().any(char::is_control)
    {
        bail!("Invalid Chromium profile directory");
    }
    let argument = quote_exec_argument(&format!("--profile-directory={profile_dir}"));
    key_file.set_string(DESKTOP_GROUP, "Exec", &format!("{exec} {argument}"));
    // D-Bus activation does not convey the explicit profile argument.
    key_file.set_boolean(DESKTOP_GROUP, "DBusActivatable", false);
    DesktopAppInfo::from_keyfile(&key_file).context("Cannot create profile Desktop Entry")
}

fn quote_exec_argument(argument: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in argument.chars() {
        match ch {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn can_add_profile(exec: &str) -> bool {
    // A KeyFile clone lacks the filename used by %k; explicit roots and -- can change profile semantics.
    !exec.contains("%k")
        && !exec.contains("--user-data-dir")
        && !exec.contains("--profile-directory")
        && !exec.contains(" -- ")
        && !exec.ends_with(" --")
}

#[derive(Clone, Copy)]
enum NativeBrowser {
    Vivaldi,
    Chrome(&'static str),
    Chromium,
}

fn native_browser(browser: &Browser, app: &DesktopAppInfo) -> Option<NativeBrowser> {
    let filename = app.filename()?;
    let exe = executable_path(app);
    if app.has_key("X-Flatpak")
        || app.has_key("X-SnapInstanceName")
        || exe.starts_with("/snap")
        || filename.to_string_lossy().contains("/snapd/")
        || filename.to_string_lossy().contains("/flatpak/")
        || app.boolean("Terminal")
        || !can_add_profile(app.string("Exec")?.as_str())
    {
        return None;
    }
    let exe_name = exe.file_name()?.to_str()?;
    match browser.desktop_id.as_deref()? {
        "vivaldi-stable.desktop" | "vivaldi.desktop"
            if matches!(exe_name, "vivaldi" | "vivaldi-stable") =>
        {
            Some(NativeBrowser::Vivaldi)
        }
        "google-chrome.desktop" | "google-chrome-stable.desktop"
            if matches!(exe_name, "google-chrome" | "google-chrome-stable") =>
        {
            Some(NativeBrowser::Chrome("google-chrome"))
        }
        "google-chrome-beta.desktop" if exe_name == "google-chrome-beta" => {
            Some(NativeBrowser::Chrome("google-chrome-beta"))
        }
        "google-chrome-unstable.desktop" if exe_name == "google-chrome-unstable" => {
            Some(NativeBrowser::Chrome("google-chrome-unstable"))
        }
        "chromium.desktop" | "chromium-browser.desktop"
            if matches!(exe_name, "chromium" | "chromium-browser") =>
        {
            Some(NativeBrowser::Chromium)
        }
        _ => None,
    }
}

struct ProfileEnvironment {
    home: Option<PathBuf>,
    xdg_config: Option<PathBuf>,
    chrome_config: Option<PathBuf>,
    chrome_user_data: Option<PathBuf>,
}

impl ProfileEnvironment {
    fn current() -> Self {
        let env_path = |name| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            home: dirs_next::home_dir(),
            xdg_config: env_path("XDG_CONFIG_HOME"),
            chrome_config: env_path("CHROME_CONFIG_HOME"),
            chrome_user_data: env_path("CHROME_USER_DATA_DIR"),
        }
    }

    fn root(&self, family: NativeBrowser) -> Option<PathBuf> {
        if !matches!(family, NativeBrowser::Vivaldi) {
            if let Some(root) = &self.chrome_user_data {
                return root.is_absolute().then(|| root.clone());
            }
        }
        let config = match family {
            NativeBrowser::Vivaldi => self.xdg_config.as_ref(),
            NativeBrowser::Chrome(_) | NativeBrowser::Chromium => {
                self.chrome_config.as_ref().or(self.xdg_config.as_ref())
            }
        };
        let base = match config {
            Some(path) => path.is_absolute().then(|| path.clone())?,
            None => self.home.as_ref()?.join(".config"),
        };
        Some(base.join(match family {
            NativeBrowser::Vivaldi => "vivaldi",
            NativeBrowser::Chrome(directory) => directory,
            NativeBrowser::Chromium => "chromium",
        }))
    }
}

pub(super) fn user_data_dir(browser: &Browser) -> Option<PathBuf> {
    let app = app_for_browser(browser).ok()?;
    let family = native_browser(browser, &app)?;
    let root = ProfileEnvironment::current().root(family)?;
    root.is_dir().then_some(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::Fixture;
    use std::os::unix::fs::PermissionsExt;

    fn desktop_file(fixture: &Fixture, filename: &str, exec: &str) -> DesktopAppInfo {
        let key_file = KeyFile::new();
        key_file.set_string(DESKTOP_GROUP, "Type", "Application");
        key_file.set_string(DESKTOP_GROUP, "Name", "合成ブラウザ");
        key_file.set_string(DESKTOP_GROUP, "Exec", exec);
        key_file.set_string(
            DESKTOP_GROUP,
            "MimeType",
            "x-scheme-handler/http;x-scheme-handler/https;",
        );
        key_file.set_string(DESKTOP_GROUP, "Path", fixture.path.to_str().unwrap());
        let path = fixture.path.join(filename);
        std::fs::write(&path, key_file.to_data().as_bytes()).unwrap();
        DesktopAppInfo::from_filename(path).unwrap()
    }

    #[test]
    fn desktop_discovery_keeps_fixed_arguments_and_excludes_self_and_hidden_entries() {
        let fixture = Fixture::new();
        let app = desktop_file(
            &fixture,
            "browser.desktop",
            "/bin/true --fixed \"two words\" %U",
        );
        let browser =
            browser_from_app(&app, "browser.desktop", Some(Path::new("/different/brows"))).unwrap();
        assert_eq!(
            browser.desktop_exec.as_deref(),
            Some("/bin/true --fixed \"two words\" %U")
        );
        assert_eq!(browser.id(), "browser.desktop");
        assert!(browser_from_app(&app, "org.gentianster.brows.desktop", None).is_none());
        assert!(browser_from_app(&app, "brows.desktop", None).is_none());
        assert!(browser_from_app(&app, "renamed.desktop", Some(Path::new("/bin/true"))).is_none());
        let hidden_path = fixture.path.join("hidden.desktop");
        std::fs::write(
            &hidden_path,
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=/bin/true %u\nHidden=true\n",
        )
        .unwrap();
        let hidden = DesktopAppInfo::from_filename(hidden_path).unwrap();
        assert!(browser_from_app(&hidden, "hidden.desktop", None).is_none());
    }

    #[test]
    fn desktop_visibility_omits_aliases_and_retains_visible_fixed_argument_variants() {
        let fixture = Fixture::new();
        let exec = "/bin/true --native %U";
        let visible = desktop_file(&fixture, "google-chrome.desktop", exec);
        let alias = desktop_file(&fixture, "com.google.Chrome.desktop", exec);
        let alias_path = alias.filename().unwrap();
        let alias_content = std::fs::read_to_string(&alias_path).unwrap();
        std::fs::write(&alias_path, format!("{alias_content}\nNoDisplay=true\n")).unwrap();
        let alias = DesktopAppInfo::from_filename(alias_path).unwrap();
        assert!(browser_from_app(&alias, "com.google.Chrome.desktop", None).is_none());
        let browser = browser_from_app(&visible, "google-chrome.desktop", None).unwrap();
        assert_eq!(browser.id(), "google-chrome.desktop");
        assert_eq!(browser.desktop_exec.as_deref(), Some(exec));

        let variant = desktop_file(
            &fixture,
            "chrome-private.desktop",
            "/bin/true --incognito %U",
        );
        let variant = browser_from_app(&variant, "chrome-private.desktop", None).unwrap();
        assert_eq!(variant.exe_path, browser.exe_path);
        assert_ne!(variant.id(), browser.id());
        assert_eq!(
            variant.desktop_exec.as_deref(),
            Some("/bin/true --incognito %U")
        );

        let restricted = desktop_file(&fixture, "restricted.desktop", exec);
        let restricted_path = restricted.filename().unwrap();
        let restricted_content = std::fs::read_to_string(&restricted_path).unwrap();
        std::fs::write(
            &restricted_path,
            format!("{restricted_content}\nOnlyShowIn=brows-tests-unregistered-desktop;\n"),
        )
        .unwrap();
        let restricted = DesktopAppInfo::from_filename(restricted_path).unwrap();
        assert!(browser_from_app(&restricted, "restricted.desktop", None).is_none());
    }

    #[test]
    fn profile_clone_preserves_original_exec_fields_and_fixed_arguments() {
        let fixture = Fixture::new();
        let exec = "/bin/true --fixed \"two words\" %U %c %%";
        let app = desktop_file(&fixture, "browser.desktop", exec);
        let profile = profile_app(&app, "Profile 1").unwrap();
        assert_eq!(
            profile.string("Exec").unwrap(),
            format!("{exec} \"--profile-directory=Profile 1\"")
        );
        assert_eq!(profile.name(), app.name());
        assert_eq!(profile.string("Path"), app.string("Path"));
        assert!(profile.supports_uris());
        assert!(!profile.boolean("DBusActivatable"));
        assert_eq!(app.string("Exec").unwrap(), exec);
    }

    #[test]
    fn gio_launch_passes_fixed_arguments_url_and_profile_without_shell_evaluation() {
        let fixture = Fixture::new();
        let capture = fixture.path.join("capture browser");
        std::fs::write(
            &capture,
            r#"#!/usr/bin/python3
import os
import sys
directory = os.path.dirname(__file__)
pending = os.path.join(directory, "captured.pending")
with open(pending, "wb") as output:
    output.write(b"\0".join(os.fsencode(arg) for arg in sys.argv[1:]))
os.replace(pending, os.path.join(directory, "captured"))
"#,
        )
        .unwrap();
        std::fs::set_permissions(&capture, std::fs::Permissions::from_mode(0o700)).unwrap();
        let exec = format!(
            "{} --fixed \"two words\" --mode=fixture %U",
            quote_exec_argument(capture.to_str().unwrap())
        );
        let app = desktop_file(&fixture, "brows-test-capture.desktop", &exec);
        let url = "https://example.invalid/path?quote=\"日本語\"&literal='single'&cmd=$(touch%20unwanted)";
        let output = fixture.path.join("captured");
        let read_arguments = || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !output.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let bytes = std::fs::read(&output).unwrap();
            bytes
                .split(|byte| *byte == 0)
                .map(|arg| std::str::from_utf8(arg).unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let browser = browser_from_app(&app, "brows-test-capture.desktop", None).unwrap();
        launch(&browser, url).unwrap();
        assert_eq!(
            read_arguments(),
            ["--fixed", "two words", "--mode=fixture", url]
        );
        for profile in ["Profile 1", "Profile 日本語\"$`\\%"] {
            std::fs::remove_file(&output).unwrap();
            launch_app(&app, Some(profile), url).unwrap();
            assert_eq!(
                read_arguments(),
                [
                    "--fixed".to_string(),
                    "two words".to_string(),
                    "--mode=fixture".to_string(),
                    url.to_string(),
                    format!("--profile-directory={profile}"),
                ]
            );
        }
        assert!(!fixture.path.join("unwanted").exists());
    }

    #[test]
    fn explicit_profile_data_roots_and_filename_field_are_not_expanded() {
        let fixture = Fixture::new();
        for exec in [
            "/bin/true --user-data-dir=/other %U",
            "/bin/true --profile-directory=Other %U",
            "/bin/true %k %U",
            "/bin/true -- %U",
        ] {
            let app = desktop_file(&fixture, "browser.desktop", exec);
            assert!(profile_app(&app, "Default").is_err());
        }
        let app = desktop_file(&fixture, "browser.desktop", "/bin/true %U");
        assert!(profile_app(&app, "Profile 1/../../other").is_err());
        assert!(profile_app(&app, "Profile 1\n--incognito").is_err());
    }

    #[test]
    fn profile_argument_encodes_literal_quotes_and_metacharacters() {
        assert_eq!(
            quote_exec_argument("--profile-directory=Profile 日本語\"$`\\%"),
            "\"--profile-directory=Profile 日本語\\\"\\$\\`\\\\%%\""
        );
    }

    #[test]
    fn native_profile_roots_honor_environment_precedence_and_channels() {
        let mut env = ProfileEnvironment {
            home: Some(PathBuf::from("/home/fixture")),
            xdg_config: None,
            chrome_config: None,
            chrome_user_data: None,
        };
        assert_eq!(
            env.root(NativeBrowser::Vivaldi).unwrap(),
            Path::new("/home/fixture/.config/vivaldi")
        );
        env.xdg_config = Some(PathBuf::from("/xdg"));
        assert_eq!(
            env.root(NativeBrowser::Chromium).unwrap(),
            Path::new("/xdg/chromium")
        );
        env.chrome_config = Some(PathBuf::from("/chrome"));
        assert_eq!(
            env.root(NativeBrowser::Chrome("google-chrome-beta"))
                .unwrap(),
            Path::new("/chrome/google-chrome-beta")
        );
        assert_eq!(
            env.root(NativeBrowser::Vivaldi).unwrap(),
            Path::new("/xdg/vivaldi")
        );
        env.chrome_user_data = Some(PathBuf::from("/explicit"));
        assert_eq!(
            env.root(NativeBrowser::Chromium).unwrap(),
            Path::new("/explicit")
        );
        assert_eq!(
            env.root(NativeBrowser::Vivaldi).unwrap(),
            Path::new("/xdg/vivaldi")
        );
        env.chrome_user_data = Some(PathBuf::from("relative"));
        assert!(env.root(NativeBrowser::Chromium).is_none());
    }

    #[test]
    fn native_desktop_ids_are_supported_but_sandbox_exports_are_not() {
        let fixture = Fixture::new();
        let binary_directory = fixture.path.join("quoted native path 日本語");
        std::fs::create_dir(&binary_directory).unwrap();
        for (id, executable) in [
            ("vivaldi-stable.desktop", "vivaldi-stable"),
            ("google-chrome.desktop", "google-chrome"),
            ("chromium.desktop", "chromium"),
        ] {
            let executable = binary_directory.join(executable);
            std::fs::write(&executable, "fixture only").unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let exec = format!("{} %U", quote_exec_argument(executable.to_str().unwrap()));
            let app = desktop_file(&fixture, id, &exec);
            let browser = browser_from_app(&app, id, None).unwrap();
            assert_eq!(Path::new(&browser.exe_path), executable);
            assert!(native_browser(&browser, &app).is_some(), "{id}");
            assert!(browser_from_app(&app, "renamed.desktop", Some(&executable)).is_none());
            let path = app.filename().unwrap();
            let original = std::fs::read_to_string(&path).unwrap();
            for sandbox_key in [
                "X-Flatpak=com.example.Browser",
                "X-SnapInstanceName=browser",
            ] {
                std::fs::write(&path, format!("{original}\n{sandbox_key}\n")).unwrap();
                let sandbox = DesktopAppInfo::from_filename(&path).unwrap();
                assert!(native_browser(&browser, &sandbox).is_none());
            }
        }
    }

    #[test]
    fn unsupported_scheme_is_rejected_before_any_app_launch() {
        let browser = Browser {
            name: "Fixture".into(),
            exe_path: "/bin/false".into(),
            profile_dir: None,
            desktop_id: None,
            desktop_file: None,
            desktop_exec: None,
        };
        for url in [
            "file:///etc/passwd",
            "http-malformed://example.com",
            "https://example.com\n",
            "http://",
        ] {
            assert!(launch(&browser, url)
                .unwrap_err()
                .to_string()
                .contains("Only HTTP"));
        }
    }
}
