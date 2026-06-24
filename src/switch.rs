use std::env;
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

    let current_bin = dirs::current_link_path().join("bin");
    let current_bin_str = current_bin.to_string_lossy().to_string();
    let already_in_path = env::var("PATH")
        .map(|p| p.split(':').any(|x| x == current_bin_str))
        .unwrap_or(false);

    if !already_in_path {
        let hint = match env::var("SHELL").as_deref() {
            Ok(s) if s.ends_with("bash") => "eval \"$(jvm init bash)\"",
            Ok(s) if s.ends_with("zsh") => "eval \"$(jvm init zsh)\"",
            Ok(s) if s.ends_with("fish") => "jvm init fish | source",
            Ok(s) if s.contains("powershell") || s.contains("pwsh") => {
                "jvm init powershell | Out-String | Invoke-Expression"
            }
            _ => "restart your shell",
        };
        println!("\nRun '{}' to update your current shell environment.", hint);
    }

    Ok(())
}


