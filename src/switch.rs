use std::env;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::dirs;

#[cfg(unix)]
#[allow(dead_code)]
fn remove_link(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(windows)]
fn remove_link(path: &Path) {
    let _ = junction::delete(path);
    let _ = fs::remove_file(path).or_else(|_| fs::remove_dir(path));
}

#[cfg(unix)]
fn create_link(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("failed to create symlink: {} -> {}", link.display(), dirs::display_path(target)))
}

#[cfg(windows)]
fn create_link(target: &Path, link: &Path) -> Result<()> {
    let result = std::os::windows::fs::symlink_dir(target, link);
    if let Err(e) = result {
        if e.raw_os_error() == Some(1314) {
            junction::create(target, link)
                .with_context(|| format!("failed to create junction: {} -> {}", link.display(), dirs::display_path(target)))?;
        } else {
            return Err(e).with_context(|| format!("failed to create symlink: {} -> {}", link.display(), dirs::display_path(target)));
        }
    }
    Ok(())
}

pub fn switch_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;

    let entry = config
        .find_by_version(version)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("version or alias not found: {}", version))?;

    let runtime_dir = dirs::runtime_dir();
    fs::create_dir_all(&runtime_dir)
        .with_context(|| format!("failed to create directory: {}", runtime_dir.display()))?;

    let current_link = dirs::current_link_path();

    // Save the old link target for potential rollback
    #[cfg(unix)]
    let old_target = fs::read_link(&current_link).ok();

    // Atomically replace the symlink first, then update the config.
    // This ensures the link is always consistent: if the config save fails,
    // the link already points to the new JDK (which is safe).
    #[cfg(unix)]
    {
        let tmp_link = current_link.with_extension("tmp");
        create_link(entry.path.as_ref(), &tmp_link)?;
        fs::rename(&tmp_link, &current_link)
            .with_context(|| format!("failed to replace symlink: {}", current_link.display()))?;
    }

    #[cfg(windows)]
    {
        remove_link(&current_link);
        create_link(entry.path.as_ref(), &current_link)?;
    }

    // Symlink is ready; now persist the config
    config.current = Some(entry.full_version.clone());
    if let Err(e) = config.save() {
        // Config persistence failed – roll back the symlink
        #[cfg(unix)]
        if let Some(old) = old_target {
            let rollback_tmp = current_link.with_extension("rollback");
            if create_link(&old, &rollback_tmp).is_ok() {
                let _ = fs::rename(&rollback_tmp, &current_link);
            }
        }
        return Err(e);
    }

    println!("Switched to JDK {} ({})", entry.full_version, dirs::display_path(entry.path.as_ref()));

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


