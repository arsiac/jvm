mod config;
mod dirs;
mod init;
mod jdk;
mod switch;

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

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

    /// Remove a registered JDK
    #[command(alias = "rm")]
    Remove {
        /// Version number, alias, or path of the JDK to remove
        target: String,
    },
}

fn cmd_add(path: &str, custom_aliases: &[String]) -> Result<()> {
    let jdk_path = Path::new(path).canonicalize()
        .with_context(|| format!("cannot access path: {}", path))?;

    if !jdk_path.join("bin").join("java").exists() {
        anyhow::bail!("{} is not a valid JDK directory (bin/java not found)", jdk_path.display());
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

fn cmd_use(version: &str) -> Result<()> {
    switch::switch_version(version)
}

fn cmd_list() -> Result<()> {
    let config = config::Config::load()?;

    if config.jdks.is_empty() {
        println!("No JDK has been added yet");
        return Ok(());
    }

    let marker_width = 3;
    let path_header = "Path";
    let version_header = "Version";
    let alias_header = "Aliases";

    let path_width = config
        .jdks
        .iter()
        .map(|e| e.path.len())
        .max()
        .unwrap_or(0)
        .max(path_header.len())
        + 2;

    let version_width = config
        .jdks
        .iter()
        .map(|e| e.full_version.len())
        .max()
        .unwrap_or(0)
        .max(version_header.len())
        + 2;

    println!(
        "  {:marker$}{:<width_path$} {:<width_ver$} {}",
        "",
        path_header,
        version_header,
        alias_header,
        marker = marker_width,
        width_path = path_width,
        width_ver = version_width
    );
    println!(
        "  {:marker$}{:-<width_path$} {:-<width_ver$} {}",
        "---",
        "",
        "",
        "---",
        marker = marker_width,
        width_path = path_width,
        width_ver = version_width
    );

    for entry in &config.jdks {
        let current_mark = if Some(&entry.full_version) == config.current.as_ref() {
            "*"
        } else {
            ""
        };
        println!(
            "  {:marker$}{:<width_path$} {:<width_ver$} {}",
            current_mark,
            entry.path,
            entry.full_version,
            entry.aliases.join(", "),
            marker = marker_width,
            width_path = path_width,
            width_ver = version_width
        );
    }

    Ok(())
}

fn cmd_remove(target: &str) -> Result<()> {
    let mut config = config::Config::load()?;
    config.remove_jdk(target)?;
    config.save()?;
    println!("Removed JDK: {}", target);
    Ok(())
}

fn cmd_init(shell: &str) -> Result<()> {
    let hook = init::generate_hook(shell)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("{}", hook);
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
        Commands::Remove { target } => cmd_remove(&target)?,
    }

    Ok(())
}
