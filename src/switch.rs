use std::fs;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::dirs;

pub fn switch_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;

    let entry = config
        .find_by_version(version)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("version or alias not found: {}", version))?;

    config.current = Some(entry.full_version.clone());
    config.save()?;

    let runtime_dir = dirs::runtime_dir();
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create directory: {}", runtime_dir.display()))?;

    let current_link = dirs::current_link_path();
    let _ = fs::remove_file(&current_link);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&entry.path, &current_link)
            .with_context(|| format!("failed to create symlink: {} -> {}", current_link.display(), entry.path))?;
    }

    println!("Switched to JDK {} ({})", entry.full_version, entry.path);
    println!("Run 'hash -r' (bash) or 'rehash' (zsh) to update command cache in the current shell.");
    Ok(())
}


