use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn create_fake_jdk(base: &std::path::Path, name: &str, version: &str) -> PathBuf {
    let jdk_dir = base.join(name);
    std::fs::create_dir_all(&jdk_dir).unwrap();
    let bin_dir = jdk_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let java_path = if cfg!(windows) {
        bin_dir.join("java.exe")
    } else {
        bin_dir.join("java")
    };
    std::fs::write(&java_path, "").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&java_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let release_content = format!(r#"JAVA_VERSION="{}""#, version);
    std::fs::write(jdk_dir.join("release"), release_content).unwrap();
    jdk_dir
}

fn jvm_cmd() -> Command {
    Command::cargo_bin("jvm").unwrap()
}

#[test]
fn test_add_and_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    let output = jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Added JDK"));

    let output = jvm_cmd()
        .arg("list")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("17.0.2"));
}

#[test]
fn test_add_with_custom_alias() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-21", "21.0.1");

    let output = jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .arg("--alias")
        .arg("lts")
        .arg("--alias")
        .arg("latest")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = jvm_cmd()
        .arg("list")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("lts"));
    assert!(stdout.contains("latest"));
}

#[test]
fn test_current_no_jdk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("current")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No JDK has been added yet"));
}

#[test]
fn test_current_no_active() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("current")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("No JDK is currently active"));
}

#[test]
fn test_use_and_current() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("use")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "use failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = jvm_cmd()
        .arg("current")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "17.0.2");
}

#[test]
fn test_remove_jdk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("remove")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = jvm_cmd()
        .arg("list")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("No JDK has been added yet"));
}

#[test]
fn test_remove_active_jdk_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    jvm_cmd()
        .arg("use")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("remove")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("currently in use"));
}

#[test]
fn test_init_all_shells() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    for shell in &["bash", "zsh", "fish", "powershell"] {
        let output = jvm_cmd()
            .arg("init")
            .arg(shell)
            .env("JVM_DIR", &jvm_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "init {} failed: {}",
            shell,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !String::from_utf8_lossy(&output.stdout).is_empty(),
            "init {} empty",
            shell
        );
    }
}

#[test]
fn test_init_unsupported_shell() {
    let output = jvm_cmd().arg("init").arg("tcsh").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported shell"));
}

#[test]
fn test_completion_bash() {
    let output = jvm_cmd().arg("completion").arg("bash").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("_jvm"));
    assert!(stdout.contains("COMPREPLY"));
}

#[test]
fn test_completion_zsh() {
    let output = jvm_cmd().arg("completion").arg("zsh").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#compdef jvm"));
}

#[test]
fn test_completion_fish() {
    let output = jvm_cmd().arg("completion").arg("fish").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("complete"));
}

#[test]
fn test_completion_powershell() {
    let output = jvm_cmd()
        .arg("completion")
        .arg("powershell")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Register-ArgumentCompleter"));
}

#[test]
fn test_completion_unsupported_shell() {
    let output = jvm_cmd().arg("completion").arg("tcsh").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported shell"));
}

#[test]
fn test_completions_alias() {
    let output = jvm_cmd().arg("completions").arg("bash").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("_jvm"));
}


#[test]
fn test_alias_add_and_remove() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("alias")
        .arg("add")
        .arg("17.0.2")
        .arg("my-jdk")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "alias add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("my-jdk"));

    let output = jvm_cmd()
        .arg("alias")
        .arg("remove")
        .arg("17.0.2")
        .arg("my-jdk")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("my-jdk"));
}

#[test]
fn test_info_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("info")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("JDK Version Manager"));
    assert!(stdout.contains("Program:"));
    assert!(stdout.contains("Paths:"));
}

#[test]
fn test_add_invalid_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("add")
        .arg("/nonexistent/path")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_multiple_jdks_and_switch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk8 = create_fake_jdk(tmp.path(), "jdk-8", "1.8.0_492");
    let jdk17 = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");
    let jdk21 = create_fake_jdk(tmp.path(), "jdk-21", "21.0.1");

    for jdk in &[&jdk8, &jdk17, &jdk21] {
        let output = jvm_cmd()
            .arg("add")
            .arg(jdk)
            .env("JVM_DIR", &jvm_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for (version, alias) in &[("1.8.0_492", "8"), ("17.0.2", "17"), ("21.0.1", "21")] {
        let output = jvm_cmd()
            .arg("use")
            .arg(alias)
            .env("JVM_DIR", &jvm_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "use {} failed: {}",
            alias,
            String::from_utf8_lossy(&output.stderr)
        );

        let output = jvm_cmd()
            .arg("current")
            .env("JVM_DIR", &jvm_dir)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            *version,
            "current mismatch after switch to {}",
            alias
        );
    }
}

#[test]
fn test_remove_by_alias() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("rm")
        .arg("17")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Removed JDK"));
}

#[test]
fn test_add_update_existing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    std::fs::write(jdk.join("release"), r#"JAVA_VERSION="17.0.3""#).unwrap();

    let output = jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "update add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_which_no_jdk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("which")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No JDK has been added yet"));
}

#[test]
fn test_which_no_active() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("which")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No JDK is currently active"));
}

#[test]
fn test_which_current() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    jvm_cmd()
        .arg("use")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("which")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), jdk.to_string_lossy());
}

#[test]
fn test_which_by_version() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("which")
        .arg("17.0.2")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), jdk.to_string_lossy());
}

#[test]
fn test_which_by_alias() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("which")
        .arg("17")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), jdk.to_string_lossy());
}

#[test]
fn test_which_nonexistent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("which")
        .arg("99.0.0")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("JDK not found"));
}

#[test]
fn test_exec_basic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("exec")
        .arg("17.0.2")
        .arg("echo")
        .arg("hello-from-jvm")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("hello-from-jvm"));
}

#[test]
fn test_exec_nonexistent_jdk() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");

    let output = jvm_cmd()
        .arg("exec")
        .arg("99.0.0")
        .arg("echo")
        .arg("test")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("JDK not found"));
}

#[test]
fn test_exec_invalid_command() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("exec")
        .arg("17.0.2")
        .arg("nonexistent-command-12345")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
#[cfg(not(windows))]
fn test_exec_java_home_set() {
    let tmp = tempfile::TempDir::new().unwrap();
    let jvm_dir = tmp.path().join("jvm");
    let jdk = create_fake_jdk(tmp.path(), "jdk-17", "17.0.2");

    jvm_cmd()
        .arg("add")
        .arg(&jdk)
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();

    let output = jvm_cmd()
        .arg("exec")
        .arg("17.0.2")
        .arg("sh")
        .arg("-c")
        .arg("echo $JAVA_HOME")
        .env("JVM_DIR", &jvm_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(stdout, jdk.to_string_lossy().to_string());
}
