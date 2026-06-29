use std::borrow::Cow;
use std::env;
use std::path::{Path, PathBuf};

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
    let base = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap().join(".config"));
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

/// Directory where jvm-managed JDK installations live.
///
/// ~/.local/share/jvm/managed/ (Linux)
/// ~/Library/Application Support/jvm/managed/ (macOS)
/// %APPDATA%/jvm/managed/ (Windows)
///
/// Override via `$JVM_DIR/managed/`.
pub fn managed_dir() -> PathBuf {
    if let Ok(val) = env::var("JVM_DIR") {
        return PathBuf::from(val).join("managed");
    }
    let base = dirs::data_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".local/share")
    });
    base.join("jvm").join("managed")
}

/// Full path to the `current` symlink.
pub fn current_link_path() -> PathBuf {
    runtime_dir().join("current")
}

/// Strip the `\\?\` prefix from Windows verbatim paths for cleaner display.
#[cfg(windows)]
pub fn display_path(path: &Path) -> Cow<'_, str> {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        Cow::Owned(stripped.to_string())
    } else {
        s
    }
}
#[cfg(not(windows))]
pub fn display_path(path: &Path) -> Cow<'_, str> {
    path.to_string_lossy()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    // --- config_dir ---

    #[test]
    #[serial]
    fn test_config_dir_with_jvm_dir() {
        env::set_var("JVM_DIR", "/custom/jvm");
        let dir = config_dir();
        assert_eq!(dir, PathBuf::from("/custom/jvm"));
        env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_config_dir_default() {
        env::remove_var("JVM_DIR");
        let dir = config_dir();
        assert!(dir.ends_with("jvm"));
    }

    // --- runtime_dir ---

    #[test]
    #[serial]
    fn test_runtime_dir_with_jvm_dir() {
        env::set_var("JVM_DIR", "/custom/jvm");
        let dir = runtime_dir();
        assert_eq!(dir, PathBuf::from("/custom/jvm"));
        env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_runtime_dir_default() {
        env::remove_var("JVM_DIR");
        let dir = runtime_dir();
        assert!(dir.ends_with("jvm"));
    }

    // --- current_link_path ---

    #[test]
    #[serial]
    fn test_current_link_path() {
        env::set_var("JVM_DIR", "/tmp/test-jvm");
        let path = current_link_path();
        assert_eq!(path, PathBuf::from("/tmp/test-jvm/current"));
        env::remove_var("JVM_DIR");
    }

    // --- managed_dir ---

    #[test]
    #[serial]
    fn test_managed_dir_with_jvm_dir() {
        env::set_var("JVM_DIR", "/custom/jvm");
        let dir = managed_dir();
        assert_eq!(dir, PathBuf::from("/custom/jvm/managed"));
        env::remove_var("JVM_DIR");
    }

    #[test]
    #[serial]
    fn test_managed_dir_default() {
        env::remove_var("JVM_DIR");
        let dir = managed_dir();
        assert!(dir.ends_with("jvm/managed"));
    }

    // --- display_path ---

    #[cfg(not(windows))]
    #[test]
    fn test_display_path_unix() {
        let p = Path::new("/usr/lib/jvm/java-17");
        assert_eq!(display_path(p), "/usr/lib/jvm/java-17");
    }

    #[cfg(windows)]
    #[test]
    fn test_display_path_windows_clean() {
        let p = Path::new("C:\\Program Files\\Java\\jdk-17");
        assert_eq!(display_path(p), "C:\\Program Files\\Java\\jdk-17");
    }

    #[cfg(windows)]
    #[test]
    fn test_display_path_strips_verbatim_prefix() {
        let p = Path::new(r"\\?\C:\Program Files\Java\jdk-17");
        assert_eq!(display_path(p), "C:\\Program Files\\Java\\jdk-17");
    }
}
