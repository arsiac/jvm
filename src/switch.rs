use std::fs;

use anyhow::{Context, Result};

use crate::config::Config;

pub fn switch_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;

    let entry = config
        .find_by_version(version)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("version or alias not found: {}", version))?;

    config.current = Some(entry.full_version.clone());
    config.save()?;

    let jvm_dir = dirs::home_dir()
        .context("cannot get $HOME directory")?
        .join(".config").join("jvm");
    fs::create_dir_all(&jvm_dir)
        .with_context(|| format!("failed to create directory: {}", jvm_dir.display()))?;

    let current_link = jvm_dir.join("current");
    let _ = fs::remove_file(&current_link);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&entry.path, &current_link)
            .with_context(|| format!("failed to create symlink: {} -> {}", current_link.display(), entry.path))?;
    }

    println!("Switched to JDK {} ({})", entry.full_version, entry.path);
    Ok(())
}


