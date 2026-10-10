use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Browser {
    pub name: String,
    pub exe_path: String,
    pub profile_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_exec: Option<String>,
}

impl Browser {
    pub fn id(&self) -> String {
        match (&self.desktop_id, &self.profile_dir) {
            (Some(id), Some(profile)) => format!("{id}#{profile}"),
            (Some(id), None) => id.clone(),
            (None, _) => self.name.clone(),
        }
    }

    pub fn launch(&self, url: &str) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            linux::launch(self, url)
        }
        #[cfg(windows)]
        {
            let mut cmd = std::process::Command::new(&self.exe_path);
            if let Some(dir) = &self.profile_dir {
                cmd.arg(format!("--profile-directory={dir}"));
            }
            cmd.arg(url).spawn()?;
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserGroup {
    pub name: String,
    pub exe_path: String,
    pub browsers: Vec<Browser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop_id: Option<String>,
}

impl BrowserGroup {
    pub fn id(&self) -> &str {
        self.desktop_id.as_deref().unwrap_or(&self.exe_path)
    }
}

pub fn detect() -> Result<Vec<Browser>> {
    #[cfg(target_os = "linux")]
    {
        linux::detect()
    }
    #[cfg(windows)]
    {
        windows::detect()
    }
}

pub fn detect_grouped() -> Result<Vec<BrowserGroup>> {
    let mut groups = Vec::new();
    for browser in detect()? {
        #[cfg(target_os = "linux")]
        let user_data_dir = linux::user_data_dir(&browser);
        #[cfg(windows)]
        let user_data_dir = windows::user_data_dir(&browser);

        let profiles = user_data_dir
            .as_deref()
            .map(read_profiles)
            .unwrap_or_default();
        groups.push(group_with_profiles(browser, profiles));
    }
    Ok(groups)
}

fn group_with_profiles(browser: Browser, profiles: Vec<(String, String)>) -> BrowserGroup {
    let browsers = if profiles.is_empty() {
        vec![browser.clone()]
    } else {
        let multiple = profiles.len() > 1;
        profiles
            .into_iter()
            .map(|(name, profile_dir)| Browser {
                name: if multiple { name } else { browser.name.clone() },
                profile_dir: Some(profile_dir),
                ..browser.clone()
            })
            .collect()
    };
    BrowserGroup {
        name: browser.name,
        exe_path: browser.exe_path,
        desktop_id: browser.desktop_id,
        browsers,
    }
}

fn read_profiles(user_data_dir: &Path) -> Vec<(String, String)> {
    try_read_profiles(user_data_dir).unwrap_or_default()
}

fn try_read_profiles(user_data_dir: &Path) -> Option<Vec<(String, String)>> {
    let local_state = std::fs::read_to_string(user_data_dir.join("Local State")).ok()?;
    let metadata: serde_json::Value = serde_json::from_str(&local_state).ok()?;
    let cache = metadata.get("profile")?.get("info_cache")?.as_object()?;
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(user_data_dir).ok()?.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Ok(dir_name) = entry.file_name().into_string() else {
            continue;
        };
        if dir_name != "Default" && !dir_name.starts_with("Profile ") {
            continue;
        }
        let Some(profile) = cache.get(&dir_name) else {
            continue;
        };
        let display = profile
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(&dir_name)
            .to_string();
        profiles.push((display, dir_name));
    }
    profiles.sort_by(|a, b| {
        (a.1 != "Default")
            .cmp(&(b.1 != "Default"))
            .then_with(|| a.1.cmp(&b.1))
    });
    Some(profiles)
}

pub fn groups_differ(a: &[BrowserGroup], b: &[BrowserGroup]) -> bool {
    a != b
}

#[derive(Clone)]
pub struct BackgroundDetect {
    pending: Arc<Mutex<Option<Vec<BrowserGroup>>>>,
}

impl BackgroundDetect {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }

    pub fn spawn(&self) {
        let pending = self.pending.clone();
        std::thread::spawn(move || {
            if let Ok(fresh) = detect_grouped() {
                *pending.lock().unwrap() = Some(fresh);
            }
        });
    }

    pub fn take(&self) -> Option<Vec<BrowserGroup>> {
        let fresh = self.pending.try_lock().ok()?.take()?;
        let to_save = fresh.clone();
        std::thread::spawn(move || {
            let _ = crate::config::Config::update(|cfg| cfg.cached_groups = to_save);
        });
        Some(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    pub(super) struct Fixture {
        pub path: PathBuf,
        _directory: tempfile::TempDir,
    }

    impl Fixture {
        pub fn new() -> Self {
            let directory = tempfile::Builder::new()
                .prefix("brows-browser-test-")
                .tempdir()
                .unwrap();
            Self {
                path: directory.path().to_path_buf(),
                _directory: directory,
            }
        }
    }

    fn browser() -> Browser {
        Browser {
            name: "Browser".into(),
            exe_path: "/browser".into(),
            profile_dir: None,
            desktop_id: Some("browser.desktop".into()),
            desktop_file: None,
            desktop_exec: Some("/browser --fixed %U".into()),
        }
    }

    #[test]
    fn profiles_decode_unicode_and_json_escapes_and_ignore_missing_directories() {
        let fixture = Fixture::new();
        for name in ["Default", "Profile 1", "Profile 2", "Unrelated"] {
            std::fs::create_dir(fixture.path.join(name)).unwrap();
        }
        std::fs::write(
            fixture.path.join("Local State"),
            r#"{
            "profile":{"info_cache":{
                "Default":{"name":"仕事 \"日本語\" \\ folder"},
                "Profile 1":{"name":"\u65e5\u672c\u8a9e"},
                "Profile 2":{"name":12},
                "Profile 3":{"name":"Missing"},
                "Unrelated":{"name":"Not a profile"}
            }}
        }"#,
        )
        .unwrap();
        assert_eq!(
            read_profiles(&fixture.path),
            vec![
                ("仕事 \"日本語\" \\ folder".into(), "Default".into()),
                ("日本語".into(), "Profile 1".into()),
                ("Profile 2".into(), "Profile 2".into()),
            ]
        );
    }

    #[test]
    fn corrupt_or_short_profile_metadata_yields_no_profiles() {
        let fixture = Fixture::new();
        std::fs::create_dir(fixture.path.join("Default")).unwrap();
        for metadata in [
            "{",
            r#"{"profile":{"info_cache":{"Default":{"name":"短い"}}}}"#,
            r#"{"profile":{"info_cache":"invalid"}}"#,
        ] {
            std::fs::write(fixture.path.join("Local State"), metadata).unwrap();
            let profiles = read_profiles(&fixture.path);
            if metadata.contains("短い") {
                assert_eq!(profiles, vec![("短い".into(), "Default".into())]);
            } else {
                assert!(profiles.is_empty());
            }
        }
    }

    #[test]
    fn renamed_added_removed_profiles_and_exec_changes_invalidate_cache() {
        let group = BrowserGroup {
            name: "Browser".into(),
            exe_path: "/browser".into(),
            browsers: vec![browser()],
            desktop_id: Some("browser.desktop".into()),
        };
        assert!(!groups_differ(
            std::slice::from_ref(&group),
            std::slice::from_ref(&group)
        ));
        let mut renamed = group.clone();
        renamed.browsers[0].name = "Renamed".into();
        assert!(groups_differ(std::slice::from_ref(&group), &[renamed]));
        let mut added = group.clone();
        let mut second = browser();
        second.profile_dir = Some("Profile 1".into());
        added.browsers.push(second);
        assert!(groups_differ(std::slice::from_ref(&group), &[added]));
        let mut removed = group.clone();
        removed.browsers.clear();
        assert!(groups_differ(std::slice::from_ref(&group), &[removed]));
        let mut changed = group.clone();
        changed.browsers[0].desktop_exec = Some("/browser --another %U".into());
        assert!(groups_differ(&[group], &[changed]));
    }

    #[test]
    fn stable_profile_identity_survives_display_name_change() {
        let mut target = browser();
        target.profile_dir = Some("Profile 1".into());
        let id = target.id();
        target.name = "Renamed".into();
        assert_eq!(target.id(), id);
        assert_eq!(id, "browser.desktop#Profile 1");
        target.desktop_id = None;
        assert_eq!(target.id(), "Renamed");
    }

    #[test]
    fn sole_profile_is_explicitly_selected_with_the_browser_group_name() {
        let group = group_with_profiles(browser(), vec![("Personal".into(), "Profile 2".into())]);
        assert_eq!(group.name, "Browser");
        assert_eq!(group.browsers[0].name, "Browser");
        assert_eq!(group.browsers[0].profile_dir.as_deref(), Some("Profile 2"));
        assert_eq!(group.browsers[0].id(), "browser.desktop#Profile 2");
    }

    #[test]
    fn legacy_cached_browser_deserializes_without_desktop_fields() {
        let browser: Browser =
            toml::from_str("name = 'Chrome'\nexe_path = 'chrome.exe'\n").unwrap();
        assert_eq!(browser.id(), "Chrome");
        assert!(browser.desktop_id.is_none());
    }
}
