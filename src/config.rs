use crate::browser::BrowserGroup;
use anyhow::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

/// load → 変更 → save を直列化するためのプロセス内ロック。
/// 複数スレッドが同時に load-modify-save すると互いのフィールドを
/// 巻き戻してしまうため、`Config::update` 経由の書き込みはここを通す
static UPDATE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub default_browser: Option<String>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub browser_order: Vec<String>,
    #[serde(default)]
    pub last_update_check: Option<u64>,
    #[serde(default)]
    pub update_available: Option<String>,
    #[serde(default)]
    pub cached_groups: Vec<BrowserGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// URLに含まれる文字列
    pub pattern: String,
    /// 使用するブラウザ名
    pub browser: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_at(&config_path())
    }

    fn load_at(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let s = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }

    fn save_at(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("config path has no parent"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    /// 最新の config を読み、変更を加えて保存する。
    /// バックグラウンドスレッドからの書き込みは必ずこれを使うこと
    /// （in-memory の Config を save() すると他スレッドの変更を巻き戻す）
    pub fn update(f: impl FnOnce(&mut Config)) -> Result<()> {
        Self::update_at(&config_path(), f)
    }

    fn update_at(path: &Path, f: impl FnOnce(&mut Config)) -> Result<()> {
        let _guard = UPDATE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("config update lock poisoned"))?;
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("config path has no parent"))?;
        std::fs::create_dir_all(parent)?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lock = options.open(path.with_extension("lock"))?;
        lock.lock_exclusive()?;
        // Settings and the resident picker are separate processes. A thread-only lock
        // lets their background saves restore stale rules or browser ordering.
        let mut cfg = Self::load_at(path)?;
        f(&mut cfg);
        cfg.save_at(path)
    }

    /// browser_order の並び順でグループをソートする（未登録は末尾）
    pub fn sort_groups(&self, groups: &mut [BrowserGroup]) {
        if self.browser_order.is_empty() {
            return;
        }
        groups.sort_by_key(|g| {
            self.browser_order
                .iter()
                .position(|o| o == g.id() || o == &g.exe_path)
                .unwrap_or(usize::MAX)
        });
    }

    /// URLにマッチするルールのブラウザ名を返す
    pub fn match_rule(&self, url: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|r| url.contains(&r.pattern))
            .map(|r| r.browser.as_str())
    }
}

pub fn config_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("brows")
        .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_config_is_reported_and_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        std::fs::write(&path, "invalid = [").unwrap();
        assert!(Config::update_at(&path, |cfg| cfg.rules.clear()).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid = [");
    }

    #[test]
    fn first_matching_url_rule_wins_and_missing_fields_keep_defaults() {
        let config: Config = toml::from_str(
            r#"
            [[rules]]
            pattern = "example.com/docs"
            browser = "docs-browser"
            [[rules]]
            pattern = "example.com"
            browser = "fallback-browser"
        "#,
        )
        .unwrap();
        assert_eq!(
            config.match_rule("https://example.com/docs/start"),
            Some("docs-browser")
        );
        assert_eq!(
            config.match_rule("https://example.com/"),
            Some("fallback-browser")
        );
        assert_eq!(config.match_rule("https://EXAMPLE.COM/"), None);
        assert!(config.cached_groups.is_empty());
    }

    #[test]
    fn isolated_update_worker() {
        let Some(path) = std::env::var_os("BROWS_TEST_CONFIG") else {
            return;
        };
        let path = PathBuf::from(path);
        let worker = std::env::var("BROWS_TEST_WORKER").unwrap();
        for index in 0..20 {
            Config::update_at(&path, |cfg| {
                cfg.rules.push(Rule {
                    pattern: format!("worker-{worker}/{index}"),
                    browser: "fixture.desktop".into(),
                })
            })
            .unwrap();
        }
    }

    #[test]
    fn independent_processes_keep_every_rule_and_unrelated_setting() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        Config::update_at(&path, |cfg| {
            cfg.default_browser = Some("keep-default".into());
            cfg.browser_order = vec!["keep-order".into()];
        })
        .unwrap();
        let executable = std::env::current_exe().unwrap();
        let mut children: Vec<_> = (0..4)
            .map(|worker| {
                std::process::Command::new(&executable)
                    .args(["--exact", "config::tests::isolated_update_worker"])
                    .env("BROWS_TEST_CONFIG", &path)
                    .env("BROWS_TEST_WORKER", worker.to_string())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for child in &mut children {
            assert!(child.wait().unwrap().success());
        }
        let config = Config::load_at(&path).unwrap();
        assert_eq!(config.rules.len(), 80);
        let patterns: std::collections::HashSet<_> =
            config.rules.iter().map(|rule| &rule.pattern).collect();
        assert_eq!(patterns.len(), 80);
        assert_eq!(config.default_browser.as_deref(), Some("keep-default"));
        assert_eq!(config.browser_order, ["keep-order"]);
    }
}
