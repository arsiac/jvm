use std::env;
use std::path::PathBuf;

/// Config directory where config.json lives.
///
/// Priority:
///   1. `$JVM_DIR` (backwards-compatible override)
///   2. Platform config directory:
///      - Linux:   `${XDG_CONFIG_HOME:-$HOME/.config}/jvm`
///      - Windows: `%APPDATA%\jvm`
pub fn config_dir() -> PathBuf {
    if let Ok(val) = env::var("JVM_DIR") {
        return PathBuf::from(val);
    }
    let base = dirs::config_dir().unwrap_or_else(|| {
        dirs::home_dir().unwrap().join(".config")
    });
    base.join("jvm")
}

/// Runtime directory where the `current` symlink/junction lives.
///
/// Priority:
///   1. `$JVM_DIR` (backwards-compatible override)
///   2. `dirs::runtime_dir()`:
///      - Linux:   `$XDG_RUNTIME_DIR/jvm`
///      - Windows: `None` (fall through)
///   3. `dirs::data_dir()`:
///      - Linux:   `~/.local/share/jvm`
///      - Windows: `%APPDATA%\jvm`
///   4. Fallback to [`config_dir`]
pub fn runtime_dir() -> PathBuf {
    if let Ok(val) = env::var("JVM_DIR") {
        return PathBuf::from(val);
    }

    if let Some(runtime) = dirs::runtime_dir() {
        return runtime.join("jvm");
    }

    if let Some(data) = dirs::data_dir() {
        return data.join("jvm");
    }

    config_dir()
}

/// Full path to the `current` symlink.
pub fn current_link_path() -> PathBuf {
    runtime_dir().join("current")
}
