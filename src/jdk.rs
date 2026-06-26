use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct JdkInfo {
    pub path: String,
    pub full_version: String,
    pub aliases: Vec<String>,
}

/// Returns the platform-appropriate Java executable path.
///
/// - Windows: `bin/java.exe`
/// - Other:   `bin/java`
pub fn java_bin_path(jdk_path: &Path) -> PathBuf {
    let bin = jdk_path.join("bin");
    if cfg!(windows) {
        bin.join("java.exe")
    } else {
        bin.join("java")
    }
}

pub fn parse_release_file(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("JAVA_VERSION=") {
            let raw = line.strip_prefix("JAVA_VERSION=")?.trim_matches('"');
            return Some(raw.to_string());
        }
    }
    None
}

pub fn parse_java_version_output(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        if line.contains("version") {
            if let Some(start) = line.find('"') {
                if let Some(end) = line.rfind('"') {
                    return Some(line[start + 1..end].to_string());
                }
            }
        }
    }
    None
}

pub fn detect_version(jdk_path: &Path) -> Result<String> {
    let release_file = jdk_path.join("release");
    if release_file.exists() {
        if let Ok(content) = fs::read_to_string(&release_file) {
            if let Some(v) = parse_release_file(&content) {
                return Ok(v);
            }
        }
    }

    let java_bin = java_bin_path(jdk_path);
    if java_bin.exists() {
        let output = Command::new(&java_bin)
            .arg("-version")
            .output()
            .context("failed to execute java -version")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(v) = parse_java_version_output(&stderr) {
            return Ok(v);
        }
    }

    Err(anyhow::anyhow!(
        "cannot detect JDK version: {}",
        jdk_path.display()
    ))
}

pub fn generate_aliases(full_version: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    aliases.push(full_version.to_string());

    let parts: Vec<&str> = full_version.splitn(3, '.').collect();
    if parts.len() >= 2 {
        let major_minor = format!("{}.{}", parts[0], parts[1]);
        if major_minor != full_version {
            aliases.push(major_minor);
        }

        if parts[0] == "1" && parts.len() >= 3 {
            let second = parts[1];
            if let Ok(parsed) = second.parse::<u32>() {
                aliases.push(parsed.to_string());
            }
        } else {
            // Java 9+ style: 17.0.2 -> alias "17"
            if let Ok(parsed) = parts[0].parse::<u32>() {
                aliases.push(parsed.to_string());
            }
        }
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- generate_aliases ---

    #[test]
    fn test_java8_aliases() {
        let aliases = generate_aliases("1.8.0_492");
        assert!(aliases.contains(&"1.8.0_492".to_string()));
        assert!(aliases.contains(&"1.8".to_string()));
        assert!(aliases.contains(&"8".to_string()));
    }

    #[test]
    fn test_java17_aliases() {
        let aliases = generate_aliases("17.0.2");
        assert!(aliases.contains(&"17.0.2".to_string()));
        assert!(aliases.contains(&"17.0".to_string()));
        assert!(aliases.contains(&"17".to_string()));
    }

    #[test]
    fn test_java11_aliases() {
        let aliases = generate_aliases("11.0.1");
        assert!(aliases.contains(&"11.0.1".to_string()));
        assert!(aliases.contains(&"11.0".to_string()));
        assert!(aliases.contains(&"11".to_string()));
    }

    #[test]
    fn test_java6_aliases() {
        let aliases = generate_aliases("1.6.0_45");
        assert!(aliases.contains(&"1.6.0_45".to_string()));
        assert!(aliases.contains(&"1.6".to_string()));
        assert!(aliases.contains(&"6".to_string()));
    }

    #[test]
    fn test_java21_aliases() {
        let aliases = generate_aliases("21.0.5");
        assert!(aliases.contains(&"21.0.5".to_string()));
        assert!(aliases.contains(&"21.0".to_string()));
        assert!(aliases.contains(&"21".to_string()));
    }

    #[test]
    fn test_java25_aliases() {
        let aliases = generate_aliases("25.0.3");
        assert!(aliases.contains(&"25.0.3".to_string()));
        assert!(aliases.contains(&"25.0".to_string()));
        assert!(aliases.contains(&"25".to_string()));
    }

    #[test]
    fn test_major_only_version() {
        let aliases = generate_aliases("17");
        assert!(aliases.contains(&"17".to_string()));
        assert_eq!(aliases.len(), 1);
    }

    #[test]
    fn test_major_minor_only_version() {
        let aliases = generate_aliases("17.0");
        assert!(aliases.contains(&"17.0".to_string()));
        assert_eq!(aliases.len(), 2);
        assert!(aliases.contains(&"17".to_string()));
    }

    #[test]
    fn test_java8_no_update() {
        let aliases = generate_aliases("1.8.0");
        assert!(aliases.contains(&"1.8.0".to_string()));
        assert!(aliases.contains(&"1.8".to_string()));
        assert!(aliases.contains(&"8".to_string()));
    }

    #[test]
    fn test_generate_aliases_dedup() {
        let aliases = generate_aliases("17.0");
        let mut sorted = aliases.clone();
        sorted.dedup();
        assert_eq!(aliases, sorted, "aliases should not contain duplicates");
    }

    #[test]
    fn test_generate_aliases_sorted() {
        let aliases = generate_aliases("17.0.2");
        let mut sorted = aliases.clone();
        sorted.sort();
        assert_eq!(aliases, sorted, "aliases should be sorted");
    }

    // --- java_bin_path ---

    #[test]
    fn test_java_bin_path_unix() {
        let p = Path::new("/usr/lib/jvm/java-17");
        let bin = java_bin_path(p);
        if cfg!(windows) {
            assert_eq!(bin, p.join("bin").join("java.exe"));
        } else {
            assert_eq!(bin, p.join("bin").join("java"));
        }
    }

    #[test]
    fn test_java_bin_path_windows() {
        let p = Path::new("C:\\Program Files\\Java\\jdk-17");
        let bin = java_bin_path(p);
        if cfg!(windows) {
            assert_eq!(bin, p.join("bin").join("java.exe"));
        } else {
            assert_eq!(bin, p.join("bin").join("java"));
        }
    }

    // --- parse_release_file ---

    #[test]
    fn test_parse_release_file_simple() {
        let content = r#"JAVA_VERSION="17.0.2""#;
        assert_eq!(parse_release_file(content), Some("17.0.2".to_string()));
    }

    #[test]
    fn test_parse_release_file_with_other_fields() {
        let content = r#"JAVA_VERSION="1.8.0_492"
JAVA_FAMILY="8"
OS_NAME="Linux""#;
        assert_eq!(parse_release_file(content), Some("1.8.0_492".to_string()));
    }

    #[test]
    fn test_parse_release_file_trailing_spaces() {
        let content = r#"  JAVA_VERSION="21.0.1"  "#;
        assert_eq!(parse_release_file(content), Some("21.0.1".to_string()));
    }

    #[test]
    fn test_parse_release_file_no_match() {
        let content = r#"JAVA_FAMILY="17"
OS_NAME="Linux""#;
        assert_eq!(parse_release_file(content), None);
    }

    #[test]
    fn test_parse_release_file_empty() {
        assert_eq!(parse_release_file(""), None);
    }

    #[test]
    fn test_parse_release_file_malformed() {
        let content = "JAVA_VERSION=broken_no_quotes";
        assert_eq!(
            parse_release_file(content),
            Some("broken_no_quotes".to_string())
        );
    }

    // --- parse_java_version_output ---

    #[test]
    fn test_parse_java_version_output_openjdk17() {
        let stderr = "openjdk version \"17.0.2\" 2022-01-18\nOpenJDK Runtime Environment ...";
        assert_eq!(
            parse_java_version_output(stderr),
            Some("17.0.2".to_string())
        );
    }

    #[test]
    fn test_parse_java_version_output_openjdk8() {
        let stderr = "openjdk version \"1.8.0_492\"\nOpenJDK Runtime Environment ...";
        assert_eq!(
            parse_java_version_output(stderr),
            Some("1.8.0_492".to_string())
        );
    }

    #[test]
    fn test_parse_java_version_output_oracle() {
        let stderr = "java version \"25.0.3\" 2025-04-15 LTS\nJava(TM) SE Runtime Environment ...";
        assert_eq!(
            parse_java_version_output(stderr),
            Some("25.0.3".to_string())
        );
    }

    #[test]
    fn test_parse_java_version_output_no_version_line() {
        let stderr = "java: command not found";
        assert_eq!(parse_java_version_output(stderr), None);
    }

    #[test]
    fn test_parse_java_version_output_empty() {
        assert_eq!(parse_java_version_output(""), None);
    }

    #[test]
    fn test_parse_java_version_output_early_access() {
        let stderr = "openjdk version \"21-ea\" 2023-09-19\nOpenJDK Runtime Environment ...";
        assert_eq!(parse_java_version_output(stderr), Some("21-ea".to_string()));
    }
}
