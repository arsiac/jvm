mod config;
mod dirs;
mod init;
mod jdk;
mod switch;

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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

    /// Remove a registered JDK
    #[command(alias = "rm")]
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
            Cell::new(&entry.path),
            version,
            Cell::new(&entry.aliases.join(", ")),
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

fn cmd_init(shell: &str) -> Result<()> {
    let hook = init::generate_hook(shell)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("{}", hook);
    Ok(())
}

fn cmd_update(target: &Option<String>, all: bool) -> Result<()> {
    match (target, all) {
        (Some(_), true) => anyhow::bail!("Cannot specify both a JDK target and --all"),
        (Some(target), false) => {
            let mut config = config::Config::load()?;
            let idx = config.jdks.iter().position(|e| {
                e.full_version == *target
                    || e.aliases.contains(target)
            }).ok_or_else(|| anyhow::anyhow!("no JDK found matching: {}", target))?;
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
                _ => println!("Updated {} JDK entr{}", updated, if updated == 1 { "y" } else { "ies" }),
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
        anyhow::bail!("path no longer exists: {}", entry.path);
    }
    if !jdk_path.join("bin").join("java").exists() {
        anyhow::bail!("not a valid JDK directory (bin/java not found): {}", entry.path);
    }

    let new_version = jdk::detect_version(jdk_path)
        .with_context(|| format!("cannot detect JDK version for {}", entry.path))?;

    if new_version == entry.full_version {
        println!("JDK {} is already up to date", entry.full_version);
        return Ok(());
    }

    let old_auto = jdk::generate_aliases(&entry.full_version);
    let custom_aliases: Vec<String> = entry.aliases.iter()
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
        if i == idx { continue; }
        for alias in &new_aliases {
            if other.aliases.contains(alias) || other.full_version == *alias {
                eprintln!("Warning: alias '{}' is also used by JDK {} ({})",
                    alias, other.full_version, other.path);
            }
        }
    }

    let old_version = config.jdks[idx].full_version.clone();
    config.jdks[idx].full_version = new_version.clone();
    config.jdks[idx].aliases = new_aliases;

    if config.current.as_deref() == Some(&old_version) {
        config.current = Some(new_version);
    }

    println!("Updated JDK: {} → {} ({})", old_version, config.jdks[idx].full_version, config.jdks[idx].path);
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
        Commands::Update { target, all } => cmd_update(&target, all)?,
    }

    Ok(())
}
