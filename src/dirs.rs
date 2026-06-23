use std::env;
use std::path::PathBuf;

/// Config directory where config.json lives.
///
/// Priority:
///   1. `$JVM_DIR` (backwards-compatible override for everything)
///   2. `$XDG_CONFIG_HOME/jvm` → default `~/.config/jvm`
pub fn config_dir() -> PathBuf {
    if let Ok(val) = env::var("JVM_DIR") {
        return PathBuf::from(val);
    }
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap().join(".config"));
    base.join("jvm")
}

/// Runtime directory where the `current` symlink lives.
///
/// Priority:
///   1. `$JVM_DIR` (backwards-compatible override)
///   2. `$XDG_RUNTIME_DIR/jvm`
///   3. fallback to [`config_dir`]
pub fn runtime_dir() -> PathBuf {
    if let Ok(val) = env::var("JVM_DIR") {
        return PathBuf::from(val);
    }
    if let Ok(val) = env::var("XDG_RUNTIME_DIR") {
        if !val.is_empty() {
            return PathBuf::from(val).join("jvm");
        }
    }
    config_dir()
}

/// Full path to the `current` symlink.
pub fn current_link_path() -> PathBuf {
    runtime_dir().join("current")
}
