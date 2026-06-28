mod completion;
mod config;
mod dirs;
mod init;
mod jdk;
mod switch;

use std::env;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use comfy_table::presets;
use comfy_table::*;

use crate::jdk::JdkInfo;

#[derive(Parser)]
#[command(name = "jvm", version, about = "JDK Version Manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a JDK installation directory
    Add {
        /// Path to the JDK installation
        path: String,

        /// Custom aliases (can be specified multiple times)
        #[arg(long)]
        alias: Vec<String>,
    },

    /// Switch to a specific JDK version
    Use {
        /// Version number or alias
        version: String,
    },

    /// List all registered JDK installations
    List,

    /// Generate shell hook script
    Init {
        /// Shell type (bash, zsh, fish)
        shell: String,
    },

    /// Show the currently active JDK version
    Current,

    /// Generate shell completion scripts
    #[command(name = "completion", visible_alias = "completions")]
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: String,
    },

    /// Remove a registered JDK
    #[command(visible_alias = "rm")]
    Remove {
        /// Version number, alias, or path of the JDK to remove
        target: String,
    },

    /// Update JDK metadata after system upgrade
    Update {
        /// JDK to update (version or alias)
        target: Option<String>,
        /// Update all registered JDKs
        #[arg(long)]
        all: bool,
    },

    /// Display detailed system-wide information
    Info,

    /// Show the path of a JDK installation
    Which {
        /// JDK version, alias, or path (defaults to current active JDK)
        target: Option<String>,
    },

    /// Run a command using a specific JDK without switching
    Exec {
        /// JDK version, alias, or path
        target: String,
        /// Command and arguments to execute
        #[arg(trailing_var_arg = true, required = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Manage JDK aliases
    #[command(subcommand)]
    Alias(AliasCommands),
}

#[derive(Subcommand)]
enum AliasCommands {
    /// Add an alias to a JDK
    Add {
        /// JDK version, alias, or path
        target: String,
        /// Alias to add
        alias: String,
    },
    /// Remove an alias from a JDK
    #[command(visible_alias = "rm")]
    Remove {
        /// JDK version, alias, or path
        target: String,
        /// Alias to remove
        alias: String,
    },
}

fn cmd_add(path: &str, custom_aliases: &[String]) -> Result<()> {
    let jdk_path = Path::new(path)
        .canonicalize()
        .with_context(|| format!("cannot access path: {}", path))?;

    #[cfg(windows)]
    let jdk_path = {
        let path_str = jdk_path.to_string_lossy().to_string();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            jdk_path
        }
    };

    if !jdk::java_bin_path(&jdk_path).exists() {
        anyhow::bail!(
            "{} is not a valid JDK directory (bin/java not found)",
            jdk_path.display()
        );
    }

    let full_version = jdk::detect_version(&jdk_path)
        .with_context(|| format!("cannot detect JDK version for {}", jdk_path.display()))?;

    let mut aliases = jdk::generate_aliases(&full_version);
    for alias in custom_aliases {
        if !aliases.contains(alias) {
            aliases.push(alias.clone());
        }
    }

    let info = JdkInfo {
        path: jdk_path.to_string_lossy().to_string(),
        full_version: full_version.clone(),
        aliases,
    };

    let mut config = config::Config::load()?;
    config.add_or_update_jdk(&info)?;
    config.save()?;

    println!("Added JDK {} ({})", full_version, jdk_path.display());
    Ok(())
}

fn cmd_current() -> Result<()> {
    let config = config::Config::load()?;
    match config.current {
        Some(v) => println!("{}", v),
        None if config.jdks.is_empty() => println!("No JDK has been added yet"),
        None => println!("No JDK is currently active"),
    }
    Ok(())
}

fn cmd_which(target: Option<&str>) -> Result<()> {
    let config = config::Config::load()?;

    let version = match target {
        Some(t) => t.to_string(),
        None => match &config.current {
            Some(v) => v.clone(),
            None if config.jdks.is_empty() => {
                anyhow::bail!("No JDK has been added yet")
            }
            None => {
                anyhow::bail!("No JDK is currently active")
            }
        },
    };

    let entry = config
        .find_by_version(&version)
        .ok_or_else(|| anyhow::anyhow!("JDK not found: {}", version))?;

    println!("{}", entry.path);
    Ok(())
}

fn cmd_exec(target: &str, command: &[String]) -> Result<()> {
    let config = config::Config::load()?;

    let entry = config
        .find_by_version(target)
        .ok_or_else(|| anyhow::anyhow!("JDK not found: {}", target))?;

    let jdk_path = Path::new(&entry.path);
    let jdk_bin = jdk::java_bin_path(jdk_path)
        .parent()
        .unwrap()
        .to_path_buf();

    let mut path_entries = Vec::new();
    path_entries.push(jdk_bin.as_os_str().to_os_string());
    if let Ok(current_path) = env::var("PATH") {
        path_entries.extend(env::split_paths(&current_path).map(|p| p.into_os_string()));
    }
    let new_path = env::join_paths(path_entries)
        .unwrap_or_else(|_| jdk_bin.to_string_lossy().into_owned().into());

    let status = std::process::Command::new(&command[0])
        .args(&command[1..])
        .env("JAVA_HOME", &entry.path)
        .env("PATH", &new_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "'{}' not found in JDK {} ({})\n\
                     Location: {}\n\
                     Hint:  Check that the command exists in:\n       {}",
                    command[0],
                    entry.full_version,
                    target,
                    entry.path,
                    dirs::display_path(&jdk_bin)
                )
            } else {
                anyhow::anyhow!("failed to execute '{}': {}", command[0], e)
            }
        })?;

    std::process::exit(status.code().unwrap_or(1));
}

fn cmd_use(version: &str) -> Result<()> {
    switch::switch_version(version)
}

fn cmd_list() -> Result<()> {
    let config = config::Config::load()?;

    if config.jdks.is_empty() {
        println!("No JDK has been added yet");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::NOTHING);
    table.set_header(vec![
        Cell::new("").add_attribute(Attribute::Bold),
        Cell::new("Path").add_attribute(Attribute::Bold),
        Cell::new("Version").add_attribute(Attribute::Bold),
        Cell::new("Aliases").add_attribute(Attribute::Bold),
    ]);

    for entry in &config.jdks {
        let is_current = Some(&entry.full_version) == config.current.as_ref();
        let marker = if is_current { "*" } else { "" };
        let version = if is_current {
            Cell::new(&entry.full_version)
                .fg(Color::Green)
                .add_attribute(Attribute::Bold)
        } else {
            Cell::new(&entry.full_version)
        };

        table.add_row(vec![
            Cell::new(marker),
            Cell::new(dirs::display_path(entry.path.as_ref())),
            version,
            Cell::new(entry.aliases.join(", ")),
        ]);
    }

    println!("{}", table);
    Ok(())
}

fn cmd_remove(target: &str) -> Result<()> {
    let mut config = config::Config::load()?;
    config.remove_jdk(target)?;
    config.save()?;
    println!("Removed JDK: {}", target);
    Ok(())
}

fn cmd_alias_add(target: &str, alias: &str) -> Result<()> {
    let mut config = config::Config::load()?;
    let version = config.add_alias(target, alias)?;
    config.save()?;
    println!("Added alias '{}' to JDK {}", alias, version);
    Ok(())
}

fn cmd_alias_remove(target: &str, alias: &str) -> Result<()> {
    let mut config = config::Config::load()?;
    let version = config.del_alias(target, alias)?;
    config.save()?;
    println!("Removed alias '{}' from JDK {}", alias, version);
    Ok(())
}

fn cmd_init(shell: &str) -> Result<()> {
    let _ = switch::heal_link();
    let hook = init::generate_hook(shell).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("{}", hook);
    Ok(())
}

fn cmd_info() -> Result<()> {
    let config = config::Config::load()?;
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;

    let config_file = config::Config::path()?;
    let runtime_dir = dirs::runtime_dir();
    let current_link = dirs::current_link_path();
    let jvm_dir = env::var("JVM_DIR").ok();

    // Check current link status
    let link_status = match std::fs::symlink_metadata(&current_link) {
        Ok(meta) if meta.file_type().is_symlink() => match std::fs::read_link(&current_link) {
            Ok(target) if target.exists() => {
                format!("valid → {}", dirs::display_path(&target))
            }
            Ok(_) => "broken".to_string(),
            Err(_) => "broken".to_string(),
        },
        Ok(meta) if meta.is_dir() => "not a symlink (directory)".to_string(),
        Ok(_) => "not a symlink".to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "not found".to_string(),
        Err(e) => format!("error: {}", e),
    };

    // Current JDK info
    let current_jdk = config
        .current
        .as_ref()
        .and_then(|v| config.jdks.iter().find(|e| e.full_version == *v));

    let title = format!("jvm: JDK Version Manager {}", version);
    let sep = "=".repeat(title.len());

    println!("{}", title);
    println!("{}", sep);
    println!();

    // Program Information
    println!(" \x1b[1mProgram:\x1b[0m");
    println!("   {:<16} {}", "Version:", version);
    println!("   {:<16} {}", "Platform:", os);
    println!();

    // Paths
    println!(" \x1b[1mPaths:\x1b[0m");
    println!(
        "   {:<16} {}",
        "Config file:",
        dirs::display_path(&config_file)
    );
    println!(
        "   {:<16} {}",
        "Runtime dir:",
        dirs::display_path(&runtime_dir)
    );
    println!(
        "   {:<16} {}",
        "Current link:",
        dirs::display_path(&current_link)
    );
    println!("   {:<16} {}", "Link status:", link_status);
    match jvm_dir {
        Some(ref d) => println!("   {:<16} {} (overrides all paths above)", "JVM_DIR:", d),
        None => println!("   {:<16} (not set)", "JVM_DIR:"),
    }
    println!();

    // JDK Status
    println!(" \x1b[1mJDK Status:\x1b[0m");
    match current_jdk {
        Some(jdk) => {
            println!("   {:<16} {}", "Current:", jdk.full_version);
            println!(
                "   {:<16} {}",
                "Location:",
                dirs::display_path(Path::new(&jdk.path))
            );
        }
        None if config.jdks.is_empty() => {
            println!("   {:<16} (no JDK registered)", "Current:");
        }
        None => {
            println!("   {:<16} (none active)", "Current:");
        }
    }
    println!("   {:<16} {}", "Registered:", config.jdks.len());
    println!();

    // Data Storage
    println!(" \x1b[1mData Storage:\x1b[0m");
    println!("   Configuration (config.json):");
    println!(
        "     {:<12} {}",
        "Location:",
        dirs::display_path(&config_file)
    );
    println!(
        "     {:<12} Registered JDK paths, version aliases, and the current active JDK version",
        "Content:"
    );
    println!();
    println!("   Runtime Link (current):");
    println!(
        "     {:<12} {}",
        "Location:",
        dirs::display_path(&current_link)
    );
    println!(
        "     {:<12} Symlink to the active JDK directory.",
        "Purpose:"
    );
    println!(
        "     {:<12} Shell hooks read this link to set JAVA_HOME and PATH.",
        ""
    );
    println!();
    println!("   \x1b[1mReset jvm completely:\x1b[0m");
    println!(
        "     rm -rf {}",
        dirs::display_path(config_file.parent().unwrap())
    );
    println!("     rm -rf {}", dirs::display_path(&runtime_dir));

    Ok(())
}

fn cmd_completions(shell: &str) -> Result<()> {
    let script = completion::generate_completion(shell, &mut Cli::command())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    print!("{}", script);
    Ok(())
}

fn cmd_update(target: &Option<String>, all: bool) -> Result<()> {
    match (target, all) {
        (Some(_), true) => anyhow::bail!("Cannot specify both a JDK target and --all"),
        (Some(target), false) => {
            let mut config = config::Config::load()?;
            let idx = config
                .jdks
                .iter()
                .position(|e| e.full_version == *target || e.aliases.contains(target))
                .ok_or_else(|| anyhow::anyhow!("no JDK found matching: {}", target))?;
            update_single_entry(&mut config, idx)?;
            config.save()?;
        }
        (None, true) => {
            let mut config = config::Config::load()?;
            let count = config.jdks.len();
            let mut updated = 0;
            for idx in 0..count {
                if let Err(e) = update_single_entry(&mut config, idx) {
                    eprintln!("Warning: {}", e);
                } else {
                    updated += 1;
                }
            }
            if updated > 0 {
                config.save()?;
            }
            match updated {
                0 => println!("No JDK entries needed updating"),
                _ => println!(
                    "Updated {} JDK entr{}",
                    updated,
                    if updated == 1 { "y" } else { "ies" }
                ),
            }
        }
        (None, false) => anyhow::bail!("Please specify a JDK to update, or use --all"),
    }
    Ok(())
}

fn update_single_entry(config: &mut config::Config, idx: usize) -> Result<()> {
    let entry = &config.jdks[idx];
    let jdk_path = Path::new(&entry.path);

    if !jdk_path.exists() {
        anyhow::bail!(
            "path no longer exists: {}",
            dirs::display_path(entry.path.as_ref())
        );
    }
    if !jdk::java_bin_path(jdk_path).exists() {
        anyhow::bail!(
            "not a valid JDK directory (bin/java not found): {}",
            dirs::display_path(entry.path.as_ref())
        );
    }

    let new_version = jdk::detect_version(jdk_path).with_context(|| {
        format!(
            "cannot detect JDK version for {}",
            dirs::display_path(entry.path.as_ref())
        )
    })?;

    if new_version == entry.full_version {
        println!("JDK {} is already up to date", entry.full_version);
        return Ok(());
    }

    let old_auto = jdk::generate_aliases(&entry.full_version);
    let custom_aliases: Vec<String> = entry
        .aliases
        .iter()
        .filter(|a| !old_auto.contains(a))
        .cloned()
        .collect();

    let mut new_aliases = jdk::generate_aliases(&new_version);
    for alias in &custom_aliases {
        if !new_aliases.contains(alias) {
            new_aliases.push(alias.clone());
        }
    }

    for (i, other) in config.jdks.iter().enumerate() {
        if i == idx {
            continue;
        }
        for alias in &new_aliases {
            if other.aliases.contains(alias) || other.full_version == *alias {
                eprintln!(
                    "Warning: alias '{}' is also used by JDK {} ({})",
                    alias, other.full_version, other.path
                );
            }
        }
    }

    let old_version = config.jdks[idx].full_version.clone();
    config.jdks[idx].full_version = new_version.clone();
    config.jdks[idx].aliases = new_aliases;

    if config.current.as_deref() == Some(&old_version) {
        config.current = Some(new_version);
    }

    println!(
        "Updated JDK: {} → {} ({})",
        old_version, config.jdks[idx].full_version, config.jdks[idx].path
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { path, alias } => cmd_add(&path, &alias)?,
        Commands::Use { version } => cmd_use(&version)?,
        Commands::List => cmd_list()?,
        Commands::Init { shell } => cmd_init(&shell)?,
        Commands::Current => cmd_current()?,
        Commands::Completions { shell } => cmd_completions(&shell)?,
        Commands::Remove { target } => cmd_remove(&target)?,
        Commands::Info => cmd_info()?,
        Commands::Update { target, all } => cmd_update(&target, all)?,
        Commands::Which { target } => cmd_which(target.as_deref())?,
        Commands::Exec { target, command } => cmd_exec(&target, &command)?,
        Commands::Alias(cmd) => match cmd {
            AliasCommands::Add { target, alias } => cmd_alias_add(&target, &alias)?,
            AliasCommands::Remove { target, alias } => cmd_alias_remove(&target, &alias)?,
        },
    }

    Ok(())
}
