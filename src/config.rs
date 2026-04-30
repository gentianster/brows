use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// デフォルトブラウザ（未選択時のフォールバック）
    pub default_browser: Option<String>,
    /// URLルール: マッチしたURLを自動で特定ブラウザで開く
    pub rules: Vec<Rule>,
}

#[derive(Debug, Serialize, Deserialize)]
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

    /// URLにマッチするルールのブラウザ名を返す
    pub fn match_rule(&self, url: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|r| url.contains(&r.pattern))
            .map(|r| r.browser.as_str())
    }
}

fn config_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("brows")
        .join("config.toml")
}