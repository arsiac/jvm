use clap::Command;
use clap_complete::{generate, Shell};

pub fn generate_completion(shell: &str, cmd: &mut Command) -> Result<String, String> {
    let shell: Shell = shell
        .parse()
        .map_err(|_| format!("unsupported shell type: {}", shell))?;

    let mut buf = Vec::new();
    generate(shell, cmd, "jvm", &mut buf);
    Ok(String::from_utf8(buf).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_command() -> Command {
        Command::new("jvm")
            .subcommand(Command::new("add").about("Add a JDK"))
            .subcommand(Command::new("use").about("Switch to a JDK"))
            .subcommand(Command::new("list").about("List JDKs"))
            .subcommand(Command::new("completion").about("Generate completions"))
    }

    #[test]
    fn test_generate_bash() {
        let script = generate_completion("bash", &mut test_command()).unwrap();
        assert!(script.contains("_jvm"));
        assert!(script.contains("COMPREPLY"));
    }

    #[test]
    fn test_generate_zsh() {
        let script = generate_completion("zsh", &mut test_command()).unwrap();
        assert!(script.contains("#compdef jvm"));
        assert!(script.contains("_jvm"));
    }

    #[test]
    fn test_generate_fish() {
        let script = generate_completion("fish", &mut test_command()).unwrap();
        assert!(script.contains("complete"));
        assert!(script.contains("jvm"));
    }

    #[test]
    fn test_generate_powershell() {
        let script = generate_completion("powershell", &mut test_command()).unwrap();
        assert!(script.contains("Register-ArgumentCompleter"));
        assert!(script.contains("jvm"));
    }

    #[test]
    fn test_generate_elvish() {
        let script = generate_completion("elvish", &mut test_command()).unwrap();
        assert!(!script.is_empty());
    }

    #[test]
    fn test_unsupported_shell() {
        let result = generate_completion("tcsh", &mut test_command());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported shell"));
    }
}
