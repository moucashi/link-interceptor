use crate::models::{Config, FavoriteEntry, HistoryEntry};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("无法解析应用数据目录")]
    MissingDataDir,
    #[error("I/O 错误：{0}")]
    Io(#[from] io::Error),
    #[error("YAML 错误：{0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone)]
pub struct Store {
    base_dir: PathBuf,
}

impl Store {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::data_dir()
            .ok_or(StorageError::MissingDataDir)?
            .join("LinkInterceptor");
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    pub fn paths(&self) -> StorePaths {
        StorePaths {
            config: self.base_dir.join("config.yaml"),
            history: self.base_dir.join("history.yaml"),
            favorites: self.base_dir.join("favorites.yaml"),
        }
    }

    pub fn load_config(&self) -> Result<Config> {
        load_or_default(&self.paths().config)
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        save_yaml(&self.paths().config, config)
    }

    pub fn load_history(&self) -> Result<Vec<HistoryEntry>> {
        load_or_default(&self.paths().history)
    }

    pub fn save_history(&self, entries: &[HistoryEntry]) -> Result<()> {
        save_yaml(&self.paths().history, entries)
    }

    pub fn load_favorites(&self) -> Result<Vec<FavoriteEntry>> {
        load_or_default(&self.paths().favorites)
    }

    pub fn save_favorites(&self, entries: &[FavoriteEntry]) -> Result<()> {
        save_yaml(&self.paths().favorites, entries)
    }
}

#[derive(Debug, Clone)]
pub struct StorePaths {
    pub config: PathBuf,
    pub history: PathBuf,
    pub favorites: PathBuf,
}

pub fn record_history(entries: &mut Vec<HistoryEntry>, url: &str) {
    let now = Utc::now();
    if let Some(entry) = entries.iter_mut().find(|entry| entry.url == url) {
        entry.last_seen_at = now;
        entry.open_count += 1;
    } else {
        entries.push(HistoryEntry::new(url.to_owned(), now));
    }
    entries.sort_by(|a, b| b.last_seen_at.cmp(&a.last_seen_at));
}

pub fn toggle_favorite(entries: &mut Vec<FavoriteEntry>, url: &str) -> bool {
    if let Some(index) = entries.iter().position(|entry| entry.url == url) {
        entries.remove(index);
        false
    } else {
        entries.push(FavoriteEntry {
            url: url.to_owned(),
            added_at: Utc::now(),
        });
        entries.sort_by(|a, b| b.added_at.cmp(&a.added_at));
        true
    }
}

pub fn is_favorite(entries: &[FavoriteEntry], url: &str) -> bool {
    entries.iter().any(|entry| entry.url == url)
}

fn load_or_default<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(T::default());
    }
    Ok(serde_yaml::from_str(&content)?)
}

fn save_yaml<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_yaml::to_string(value)?;
    fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_deduplicates_and_counts() {
        let mut entries = Vec::new();
        record_history(&mut entries, "https://example.com");
        record_history(&mut entries, "https://example.com");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].open_count, 2);
    }

    #[test]
    fn favorite_toggle_adds_and_removes() {
        let mut entries = Vec::new();
        assert!(toggle_favorite(&mut entries, "https://example.com"));
        assert!(is_favorite(&entries, "https://example.com"));
        assert!(!toggle_favorite(&mut entries, "https://example.com"));
        assert!(!is_favorite(&entries, "https://example.com"));
    }

    #[test]
    fn yaml_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::with_base_dir(dir.path().to_path_buf()).unwrap();
        let config = Config {
            custom_apps: vec![],
            domain_rules: vec![],
        };
        store.save_config(&config).unwrap();
        assert_eq!(store.load_config().unwrap(), config);
    }
}
