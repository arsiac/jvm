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
        fs::write(&path, content)
            .with_context(|| format!("failed to write config: {}", path.display()))?;
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
        for entry in &self.jdks {
            if entry.path == target || entry.path.ends_with(target) {
                return Some(entry);
            }
        }
        None
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
