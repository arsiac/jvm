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
    std::os::unix::fs::symlink(target, link).with_context(|| {
        format!(
            "failed to create symlink: {} -> {}",
            link.display(),
            dirs::display_path(target)
        )
    })
}

#[cfg(windows)]
fn create_link(target: &Path, link: &Path) -> Result<()> {
    let result = std::os::windows::fs::symlink_dir(target, link);
    if let Err(e) = result {
        if e.raw_os_error() == Some(1314) {
            junction::create(target, link).with_context(|| {
                format!(
                    "failed to create junction: {} -> {}",
                    link.display(),
                    dirs::display_path(target)
                )
            })?;
        } else {
            return Err(e).with_context(|| {
                format!(
                    "failed to create symlink: {} -> {}",
                    link.display(),
                    dirs::display_path(target)
                )
            });
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
    config.save().inspect_err(|_| {
        // Config persistence failed – roll back the symlink
        #[cfg(unix)]
        if let Some(old) = old_target {
            let rollback_tmp = current_link.with_extension("rollback");
            if create_link(&old, &rollback_tmp).is_ok() {
                let _ = fs::rename(&rollback_tmp, &current_link);
            }
        }

        #[cfg(windows)]
        if let Some(ref old) = old_target {
            let _ = remove_link(&current_link);
            if std::os::windows::fs::symlink_dir(old, &current_link).is_err() {
                let _ = junction::create(old, &current_link);
            }
        }
    })?;

    println!(
        "Switched to JDK {} ({})",
        entry.full_version,
        dirs::display_path(entry.path.as_ref())
    );

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

/// Restore the current symlink from the persisted config if it's missing or broken.
pub fn heal_link() -> Result<()> {
    let config = Config::load()?;
    let link = dirs::current_link_path();

    // Check if the link already exists and points to a valid target
    if let Ok(meta) = fs::symlink_metadata(&link) {
        if meta.file_type().is_symlink() {
            if let Ok(target) = fs::read_link(&link) {
                if target.exists() {
                    return Ok(());
                }
            }
        }
    }

    // No active version to restore
    let version = match config.current {
        Some(ref v) => v,
        None => return Ok(()),
    };

    let entry = match config.find_by_version(version) {
        Some(e) => e.clone(),
        None => {
            eprintln!(
                "warning: current JDK '{}' not found in config, cannot restore symlink",
                version
            );
            return Ok(());
        }
    };

    let runtime_dir = dirs::runtime_dir();
    fs::create_dir_all(&runtime_dir).with_context(|| {
        format!(
            "failed to create runtime directory: {}",
            runtime_dir.display()
        )
    })?;

    create_link(entry.path.as_ref(), &link)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JdkEntry;
    use serial_test::serial;
    use tempfile::TempDir;

    fn setup_config_with_jdks(tmp: &TempDir) -> (Config, String) {
        let jdk_dir = tmp.path().join("jdk-17");
        std::fs::create_dir_all(&jdk_dir).unwrap();
        std::fs::write(jdk_dir.join("release"), r#"JAVA_VERSION="17.0.2""#).unwrap();
        let jdk_path_str = jdk_dir.to_string_lossy().to_string();

        let config = Config {
            current: None,
            jdks: vec![JdkEntry {
                path: jdk_path_str.clone(),
                full_version: "17.0.2".to_string(),
                aliases: vec!["17".to_string(), "17.0".to_string()],
            }],
        };
        (config, jdk_path_str)
    }

    #[test]
    #[serial]
    fn test_switch_version_success() {
        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("jvm");
        std::env::set_var("JVM_DIR", &jvm_dir);

        let (cfg, _) = setup_config_with_jdks(&tmp);
        crate::config::Config {
            current: cfg.current.clone(),
            jdks: cfg.jdks.clone(),
        }
        .save()
        .unwrap();

        let result = switch_version("17.0.2");
        assert!(result.is_ok(), "switch_version failed: {:?}", result.err());

        let current_link = jvm_dir.join("current");
        assert!(current_link.exists() || cfg!(windows));

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.current, Some("17.0.2".to_string()));

        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_switch_version_not_found() {
        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("jvm");
        std::env::set_var("JVM_DIR", &jvm_dir);

        let cfg = Config {
            current: None,
            jdks: vec![],
        };
        cfg.save().unwrap();

        let result = switch_version("nonexistent");
        assert!(result.is_err());

        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn test_switch_version_atomic_replacement() {
        use std::path::PathBuf;

        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("jvm");
        std::env::set_var("JVM_DIR", &jvm_dir);
        std::fs::create_dir_all(&jvm_dir).unwrap();

        let (cfg, jdk_path) = setup_config_with_jdks(&tmp);
        crate::config::Config {
            current: None,
            jdks: cfg.jdks,
        }
        .save()
        .unwrap();

        switch_version("17.0.2").unwrap();

        let current_link = jvm_dir.join("current");
        assert!(current_link.is_symlink());
        let target = std::fs::read_link(&current_link).unwrap();
        assert_eq!(target, PathBuf::from(&jdk_path));

        std::env::remove_var("JVM_DIR");
    }

    #[test]
    #[cfg(unix)]
    #[serial]
    fn test_switch_version_updates_current_in_config() {
        let tmp = TempDir::new().unwrap();
        let jvm_dir = tmp.path().join("jvm");
        std::env::set_var("JVM_DIR", &jvm_dir);

        let (cfg, _) = setup_config_with_jdks(&tmp);
        crate::config::Config {
            current: None,
            jdks: cfg.jdks,
        }
        .save()
        .unwrap();

        switch_version("17.0.2").unwrap();

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.current, Some("17.0.2".to_string()));

        std::env::remove_var("JVM_DIR");
    }
}
