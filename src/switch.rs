use std::env;
use std::fs;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::dirs;

#[cfg(windows)]
fn display_path(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}
#[cfg(not(windows))]
fn display_path(path: &str) -> &str { path }

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

    // Remove any existing item: junction, directory, file, or symlink
    #[cfg(windows)]
    let _ = junction::delete(&current_link);
    let _ = fs::remove_file(&current_link);
    let _ = fs::remove_dir(&current_link);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&entry.path, &current_link)
            .with_context(|| format!("failed to create symlink: {} -> {}", current_link.display(), display_path(&entry.path)))?;
    }

    #[cfg(windows)]
    {
        let result = std::os::windows::fs::symlink_dir(&entry.path, &current_link);
        if let Err(e) = result {
            if e.raw_os_error() == Some(1314) {
                // No symlink privilege: fall back to junction
                junction::create(&entry.path, &current_link)
                    .with_context(|| format!("failed to create junction: {} -> {}", current_link.display(), display_path(&entry.path)))?;
            } else {
                return Err(e).with_context(|| format!("failed to create symlink: {} -> {}", current_link.display(), display_path(&entry.path)));
            }
        }
    }

    println!("Switched to JDK {} ({})", entry.full_version, display_path(&entry.path));

    let current_bin = dirs::current_link_path().join("bin");
    let current_bin_str = current_bin.to_string_lossy().to_string();
    let already_in_path = env::var("PATH")
        .map(|p| std::env::split_paths(&p).any(|x| x == current_bin_str))
        .unwrap_or(false);

    if !already_in_path {
        let hint = if cfg!(windows) {
            "jvm init powershell | Out-String | Invoke-Expression"
        } else {
            match env::var("SHELL").as_deref() {
                Ok(s) if s.ends_with("bash") => "eval \"$(jvm init bash)\"",
                Ok(s) if s.ends_with("zsh") => "eval \"$(jvm init zsh)\"",
                Ok(s) if s.ends_with("fish") => "jvm init fish | source",
                _ => "restart your shell",
            }
        };
        println!("\nRun '{}' to update your current shell environment.", hint);
    }

    Ok(())
}


