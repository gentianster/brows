use anyhow::Result;
use crate::browser::BrowserGroup;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// load → 変更 → save を直列化するためのプロセス内ロック。
/// 複数スレッドが同時に load-modify-save すると互いのフィールドを
/// 巻き戻してしまうため、`Config::update` 経由の書き込みはここを通す
static UPDATE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize, Default)]
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
        let path = config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let s = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&s)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// 最新の config を読み、変更を加えて保存する。
    /// バックグラウンドスレッドからの書き込みは必ずこれを使うこと
    /// （in-memory の Config を save() すると他スレッドの変更を巻き戻す）
    pub fn update(f: impl FnOnce(&mut Config)) -> Result<()> {
        let _guard = UPDATE_LOCK.lock().unwrap();
        let mut cfg = Config::load().unwrap_or_default();
        f(&mut cfg);
        cfg.save()
    }

    /// browser_order の並び順でグループをソートする（未登録は末尾）
    pub fn sort_groups(&self, groups: &mut [BrowserGroup]) {
        if self.browser_order.is_empty() {
            return;
        }
        groups.sort_by_key(|g| {
            self.browser_order.iter().position(|o| o == &g.exe_path).unwrap_or(usize::MAX)
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