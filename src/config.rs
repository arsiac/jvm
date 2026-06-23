use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
        let home = dirs::home_dir().context("cannot get $HOME directory")?;
        let dir = home.join(".config").join("jvm");
        Ok(dir.join("config.json"))
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

    pub fn find_by_version(&self, version: &str) -> Option<&JdkEntry> {
        for entry in &self.jdks {
            if entry.full_version == version || entry.aliases.contains(&version.to_string()) {
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
}
