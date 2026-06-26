use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::dirs;
use crate::jdk::JdkInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdkEntry {
    pub path: String,
    pub full_version: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub current: Option<String>,
    pub jdks: Vec<JdkEntry>,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        Ok(dirs::config_dir().join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config: {}", path.display()))?;
            let config: Config = serde_json::from_str(&content)
                .with_context(|| format!("failed to parse config: {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Config {
                current: None,
                jdks: Vec::new(),
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));
        fs::write(&tmp_path, &content)
            .with_context(|| format!("failed to write config: {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path)
            .with_context(|| format!("failed to rename config: {} -> {}", tmp_path.display(), path.display()))?;
        Ok(())
    }

    pub fn add_or_update_jdk(&mut self, info: &JdkInfo) -> Result<bool> {
        if let Some(existing) = self.jdks.iter_mut().find(|e| e.path == info.path) {
            existing.full_version = info.full_version.clone();
            let merged: HashSet<String> = existing
                .aliases
                .iter()
                .cloned()
                .chain(info.aliases.iter().cloned())
                .collect();
            existing.aliases = merged.into_iter().collect();
            existing.aliases.sort();
            return Ok(false);
        }

        let existing_aliases: HashSet<&str> = self
            .jdks
            .iter()
            .flat_map(|e| e.aliases.iter().map(|a| a.as_str()))
            .collect();

        let new_aliases: Vec<String> = info
            .aliases
            .iter()
            .filter(|a| !existing_aliases.contains(a.as_str()))
            .cloned()
            .collect();

        if new_aliases.len() != info.aliases.len() {
            for alias in info.aliases.iter() {
                if !new_aliases.contains(alias) {
                    if let Some(other) = self.jdks.iter().find(|e| e.aliases.contains(alias)) {
                        eprintln!("warning: alias {} is already used by {}", alias, other.path);
                    }
                }
            }
        }

        self.jdks.push(JdkEntry {
            path: info.path.clone(),
            full_version: info.full_version.clone(),
            aliases: new_aliases,
        });

        Ok(true)
    }

    pub fn find_by_version(&self, target: &str) -> Option<&JdkEntry> {
        for entry in &self.jdks {
            if entry.full_version == target || entry.aliases.contains(&target.to_string()) {
                return Some(entry);
            }
        }
        self.jdks.iter().find(|entry| entry.path == target || entry.path.ends_with(target))
    }

    pub fn remove_jdk(&mut self, version_or_path: &str) -> Result<()> {
        let pos = self.jdks.iter().position(|e| {
            e.full_version == version_or_path
                || e.aliases.contains(&version_or_path.to_string())
                || e.path == version_or_path
        }).ok_or_else(|| anyhow::anyhow!("no JDK found matching: {}", version_or_path))?;

        let entry = &self.jdks[pos];
        if self.current.as_deref() == Some(&entry.full_version) {
            anyhow::bail!("JDK {} is currently in use, switch to another version first", entry.full_version);
        }

        self.jdks.remove(pos);
        Ok(())
    }

    pub fn add_alias(&mut self, target: &str, new_alias: &str) -> Result<String> {
        let idx = self.jdks.iter().position(|e| {
            e.full_version == target
                || e.aliases.contains(&target.to_string())
                || e.path == target
                || e.path.ends_with(target)
        }).ok_or_else(|| anyhow::anyhow!("no JDK found matching: {}", target))?;

        let entry_path = self.jdks[idx].path.clone();
        let entry_version = self.jdks[idx].full_version.clone();

        if self.jdks[idx].aliases.contains(&new_alias.to_string()) {
            anyhow::bail!("alias '{}' already exists for JDK {}", new_alias, entry_version);
        }

        for other in &self.jdks {
            if other.path == entry_path { continue; }
            if other.full_version == new_alias || other.aliases.contains(&new_alias.to_string()) {
                anyhow::bail!("alias '{}' is already used by JDK {} ({})",
                    new_alias, other.full_version, other.path);
            }
        }

        self.jdks[idx].aliases.push(new_alias.to_string());
        self.jdks[idx].aliases.sort();
        Ok(entry_version)
    }

    pub fn del_alias(&mut self, target: &str, alias: &str) -> Result<String> {
        let entry = self.jdks.iter_mut()
            .find(|e| {
                e.full_version == target
                    || e.aliases.contains(&target.to_string())
                    || e.path == target
                    || e.path.ends_with(target)
            })
            .ok_or_else(|| anyhow::anyhow!("no JDK found matching: {}", target))?;

        if alias == entry.full_version {
            anyhow::bail!("cannot remove the primary version identifier '{}'", alias);
        }

        if !entry.aliases.contains(&alias.to_string()) {
            anyhow::bail!("alias '{}' not found for JDK {}", alias, entry.full_version);
        }

        entry.aliases.retain(|a| a != alias);
        Ok(entry.full_version.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jdk::JdkInfo;
    use serial_test::serial;
    use tempfile::TempDir;

    fn make_entry(path: &str, version: &str, aliases: &[&str]) -> JdkEntry {
        JdkEntry {
            path: path.to_string(),
            full_version: version.to_string(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_info(path: &str, version: &str, aliases: &[&str]) -> JdkInfo {
        JdkInfo {
            path: path.to_string(),
            full_version: version.to_string(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sample_config() -> Config {
        Config {
            current: Some("17.0.2".to_string()),
            jdks: vec![
                make_entry("/usr/lib/jvm/java-8", "1.8.0_492", &["1.8", "8"]),
                make_entry("/usr/lib/jvm/java-17", "17.0.2", &["17.0", "17"]),
                make_entry("/usr/lib/jvm/java-21", "21.0.1", &["21.0", "21"]),
            ],
        }
    }

    // --- find_by_version ---

    #[test]
    fn test_find_by_full_version() {
        let cfg = sample_config();
        let entry = cfg.find_by_version("17.0.2").unwrap();
        assert_eq!(entry.full_version, "17.0.2");
    }

    #[test]
    fn test_find_by_alias() {
        let cfg = sample_config();
        let entry = cfg.find_by_version("8").unwrap();
        assert_eq!(entry.full_version, "1.8.0_492");
    }

    #[test]
    fn test_find_by_exact_path() {
        let cfg = sample_config();
        let entry = cfg.find_by_version("/usr/lib/jvm/java-17").unwrap();
        assert_eq!(entry.full_version, "17.0.2");
    }

    #[test]
    fn test_find_by_path_suffix() {
        let cfg = sample_config();
        let entry = cfg.find_by_version("java-21").unwrap();
        assert_eq!(entry.full_version, "21.0.1");
    }

    #[test]
    fn test_find_by_nonexistent() {
        let cfg = sample_config();
        assert!(cfg.find_by_version("99.0.0").is_none());
    }

    // --- add_or_update_jdk ---

    #[test]
    fn test_add_new_jdk() {
        let mut cfg = sample_config();
        let info = make_info("/opt/java-11", "11.0.1", &["11.0", "11"]);
        let is_new = cfg.add_or_update_jdk(&info).unwrap();
        assert!(is_new);
        assert_eq!(cfg.jdks.len(), 4);
        assert_eq!(cfg.jdks[3].full_version, "11.0.1");
    }

    #[test]
    fn test_update_existing_jdk() {
        let mut cfg = sample_config();
        let info = make_info("/usr/lib/jvm/java-17", "17.0.3", &["17.0", "17"]);
        let is_new = cfg.add_or_update_jdk(&info).unwrap();
        assert!(!is_new);
        assert_eq!(cfg.jdks[1].full_version, "17.0.3");
    }

    #[test]
    fn test_add_jdk_alias_collision_warning() {
        let mut cfg = sample_config();
        // alias "8" already taken by java-8
        let info = make_info("/opt/java-legacy", "1.8.0_100", &["1.8", "8"]);
        cfg.add_or_update_jdk(&info).unwrap();
        // Should still be added but with warning (alias 8 should be filtered)
        let entry = cfg.jdks.last().unwrap();
        assert!(!entry.aliases.contains(&"8".to_string()));
    }

    #[test]
    fn test_add_jdk_merge_aliases() {
        let mut cfg = sample_config();
        let info = make_info("/usr/lib/jvm/java-17", "17.0.5", &["custom-alias", "extra"]);
        cfg.add_or_update_jdk(&info).unwrap();
        let entry = cfg.jdks.iter().find(|e| e.path == "/usr/lib/jvm/java-17").unwrap();
        assert!(entry.aliases.contains(&"custom-alias".to_string()));
        assert!(entry.aliases.contains(&"extra".to_string()));
        // Original auto aliases from existing entry should still be present (merged)
        assert!(entry.aliases.contains(&"17.0".to_string()));
    }

    // --- remove_jdk ---

    #[test]
    fn test_remove_by_version() {
        let mut cfg = sample_config();
        cfg.remove_jdk("21.0.1").unwrap();
        assert_eq!(cfg.jdks.len(), 2);
        assert!(cfg.jdks.iter().all(|e| e.full_version != "21.0.1"));
    }

    #[test]
    fn test_remove_by_alias() {
        let mut cfg = sample_config();
        cfg.remove_jdk("8").unwrap();
        assert_eq!(cfg.jdks.len(), 2);
    }

    #[test]
    fn test_remove_by_path() {
        let mut cfg = sample_config();
        cfg.remove_jdk("/usr/lib/jvm/java-21").unwrap();
        assert_eq!(cfg.jdks.len(), 2);
    }

    #[test]
    fn test_remove_current_jdk_rejected() {
        let mut cfg = sample_config();
        let result = cfg.remove_jdk("17.0.2");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("currently in use"));
    }

    #[test]
    fn test_remove_nonexistent_jdk() {
        let mut cfg = sample_config();
        let result = cfg.remove_jdk("99.0.0");
        assert!(result.is_err());
    }

    // --- add_alias ---

    #[test]
    fn test_add_alias_success() {
        let mut cfg = sample_config();
        let version = cfg.add_alias("21.0.1", "lts").unwrap();
        assert_eq!(version, "21.0.1");
        let entry = cfg.find_by_version("lts").unwrap();
        assert_eq!(entry.full_version, "21.0.1");
    }

    #[test]
    fn test_add_alias_duplicate() {
        let mut cfg = sample_config();
        let result = cfg.add_alias("17.0.2", "17");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_alias_used_by_other() {
        let mut cfg = sample_config();
        // alias "8" is used by java-8
        let result = cfg.add_alias("17.0.2", "8");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already used"));
    }

    // --- del_alias ---

    #[test]
    fn test_del_alias_success() {
        let mut cfg = sample_config();
        let version = cfg.del_alias("21.0.1", "21.0").unwrap();
        assert_eq!(version, "21.0.1");
        let entry = cfg.find_by_version("21.0.1").unwrap();
        assert!(!entry.aliases.contains(&"21.0".to_string()));
    }

    #[test]
    fn test_del_alias_primary_version_rejected() {
        let mut cfg = sample_config();
        let result = cfg.del_alias("17.0.2", "17.0.2");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot remove the primary"));
    }

    #[test]
    fn test_del_alias_nonexistent() {
        let mut cfg = sample_config();
        let result = cfg.del_alias("17.0.2", "nonexistent");
        assert!(result.is_err());
    }

    // --- load / save ---

    #[test]
    #[serial]
    fn test_save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("jvm").join("config.json");
        // Override config dir via JVM_DIR to point to temp
        std::env::set_var("JVM_DIR", tmp.path().join("jvm"));
        let cfg = sample_config();
        cfg.save().unwrap();
        assert!(config_path.exists());
        let loaded = Config::load().unwrap();
        assert_eq!(loaded.current, cfg.current);
        assert_eq!(loaded.jdks.len(), cfg.jdks.len());
        assert_eq!(loaded.jdks[1].full_version, "17.0.2");
        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_load_empty_config() {
        let tmp = TempDir::new().unwrap();
        std::env::set_var("JVM_DIR", tmp.path().join("jvm"));
        let cfg = Config::load().unwrap();
        assert!(cfg.current.is_none());
        assert!(cfg.jdks.is_empty());
        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_load_corrupted_json() {
        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("jvm");
        std::fs::create_dir_all(&jvm_dir).unwrap();
        std::fs::write(jvm_dir.join("config.json"), "not valid json").unwrap();
        std::env::set_var("JVM_DIR", &jvm_dir);
        let result = Config::load();
        assert!(result.is_err());
        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_save_creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("nested").join("jvm");
        std::env::set_var("JVM_DIR", &jvm_dir);
        let cfg = Config {
            current: None,
            jdks: vec![],
        };
        cfg.save().unwrap();
        assert!(jvm_dir.join("config.json").exists());
        std::env::remove_var("JVM_DIR");
    }
}
