use std::path::Path;
use std::process::Command;
use std::fs;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct JdkInfo {
    pub path: String,
    pub full_version: String,
    pub aliases: Vec<String>,
}

pub fn detect_version(jdk_path: &Path) -> Result<String> {
    let release_file = jdk_path.join("release");
    if release_file.exists() {
        if let Ok(content) = fs::read_to_string(&release_file) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("JAVA_VERSION=") {
                    let raw = line
                        .strip_prefix("JAVA_VERSION=")
                        .unwrap()
                        .trim_matches('"');
                    return Ok(raw.to_string());
                }
            }
        }
    }

    let java_bin = jdk_path.join("bin").join("java");
    if java_bin.exists() {
        let output = Command::new(&java_bin)
            .arg("-version")
            .output()
            .context("failed to execute java -version")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if line.contains("version") {
                if let Some(start) = line.find('"') {
                    if let Some(end) = line.rfind('"') {
                        return Ok(line[start + 1..end].to_string());
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!("cannot detect JDK version: {}", jdk_path.display()))
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
}
