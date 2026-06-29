use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;

use crate::config;
use crate::dirs;
use crate::jdk::{self, JdkInfo};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistSource {
    Temurin,
}

impl DistSource {
    fn api_base(&self) -> &'static str {
        match self {
            DistSource::Temurin => "https://api.adoptium.net/v3",
        }
    }

    fn vendor(&self) -> &'static str {
        match self {
            DistSource::Temurin => "eclipse",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            DistSource::Temurin => "Temurin",
        }
    }
}

impl FromStr for DistSource {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "temurin" => Ok(DistSource::Temurin),
            other => Err(format!("unsupported distribution: {other}")),
        }
    }
}

pub struct JdkInstallOpts {
    pub version: Option<String>,
    pub dist: DistSource,
    pub install_path: Option<PathBuf>,
    pub proxy: Option<String>,
    pub aliases: Vec<String>,
    pub dry_run: bool,
}

#[derive(Deserialize)]
struct AdoptiumRelease {
    version_data: VersionData,
    binaries: Vec<Binary>,
}

#[derive(Deserialize)]
struct VersionData {
    semver: String,
    openjdk_version: Option<String>,
}

#[derive(Deserialize)]
struct Binary {
    #[serde(rename = "package")]
    pkg: Package,
}

#[derive(Deserialize)]
struct Package {
    link: String,
    name: String,
    size: Option<i64>,
}

#[derive(Deserialize)]
struct AvailableReleases {
    #[serde(rename = "available_releases")]
    releases: Vec<i32>,
    #[serde(rename = "available_lts_releases")]
    lts_releases: Vec<i32>,
}

fn current_os() -> &'static str {
    match env::consts::OS {
        "linux" => "linux",
        "macos" => "mac",
        "windows" => "windows",
        _ => "linux",
    }
}

fn current_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "arm" => "arm",
        _ => "x64",
    }
}

pub fn resolve_proxy(cli_proxy: Option<&str>) -> Option<String> {
    let raw = cli_proxy
        .map(|s| s.to_string())
        .or_else(|| env::var("HTTPS_PROXY").or(env::var("https_proxy")).ok())
        .or_else(|| env::var("HTTP_PROXY").or(env::var("http_proxy")).ok())
        .or_else(|| env::var("ALL_PROXY").or(env::var("all_proxy")).ok())?;

    if raw.is_empty() {
        return None;
    }

    // Add http:// scheme if missing
    if !raw.contains("://") {
        Some(format!("http://{raw}"))
    } else {
        Some(raw)
    }
}

fn build_client(proxy: Option<&str>) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("jvm/", env!("CARGO_PKG_VERSION")));
    if let Some(proxy_url) = proxy {
        match reqwest::Proxy::all(proxy_url) {
            Ok(p) => builder = builder.proxy(p),
            Err(e) => {
                eprintln!("warning: ignoring invalid proxy URL '{proxy_url}': {e}");
            }
        }
    }
    Ok(builder.build()?)
}

pub fn list_versions(opts: &JdkInstallOpts) -> Result<()> {
    let proxy = opts.proxy.as_deref();
    let client = build_client(proxy)?;

    match &opts.version {
        None => {
            let url = format!("{}/info/available_releases", opts.dist.api_base());
            let resp = client.get(&url).send()?;
            let data: AvailableReleases = resp.json()?;
            let lts_set: std::collections::HashSet<i32> = data.lts_releases.into_iter().collect();
            println!("Available JDK major versions from {}:", opts.dist.name());
            for v in &data.releases {
                let lts = if lts_set.contains(v) { " (LTS)" } else { "" };
                println!("  {v}{lts}");
            }
        }
        Some(ver) => {
            let major: i32 = match ver.parse() {
                Ok(v) => v,
                Err(_) => bail!("invalid major version: {ver}"),
            };
            let url = format!(
                "{}/assets/feature_releases/{major}/ga?architecture={}&image_type=jdk&os={}&project=jdk&vendor={}&sort_method=DEFAULT&sort_order=DESC&page=0&page_size=20",
                opts.dist.api_base(), current_arch(), current_os(), opts.dist.vendor(),
            );
            let resp = client.get(&url).send()?;
            let releases: Vec<AdoptiumRelease> = resp.json()?;
            if releases.is_empty() {
                println!("No releases found for JDK {major}");
                return Ok(());
            }
            println!("Available JDK {major} releases from {}:", opts.dist.name());
            for r in &releases {
                let bin = &r.binaries[0];
                let size = bin.pkg.size.map(format_size).unwrap_or_default();
                let lts = if r.version_data.semver.contains(".LTS") {
                    "  (LTS)"
                } else {
                    ""
                };
                println!("  {}{}  ({})", r.version_data.semver, lts, size);
            }
        }
    }
    Ok(())
}

pub fn install_jdk(opts: JdkInstallOpts) -> Result<()> {
    let proxy = opts.proxy.as_deref();

    let (semver, download_url, archive_name, size_hint) = resolve_version(&opts, proxy)?;

    let install_dir = match &opts.install_path {
        Some(p) => {
            let p = PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                env::current_dir()?.join(p)
            }
        }
        None => dirs::managed_dir().join(format!("jdk-{semver}")),
    };

    if !opts.dry_run && install_dir.exists() {
        bail!(
            "JDK {semver} already installed at {}",
            install_dir.display()
        );
    }

    if opts.dry_run {
        println!("Distribution:  {}", opts.dist.name());
        println!("Version:       {semver}");
        println!("Archive:       {archive_name}");
        println!("Size:          {}", format_size(size_hint.unwrap_or(0)));
        println!("Download URL:  {download_url}");
        println!("Install path:  {}", install_dir.display());
        return Ok(());
    }

    let parent = install_dir.parent().unwrap();
    fs::create_dir_all(parent)?;

    let archive_path = parent.join(format!(".tmp_{semver}_{archive_name}"));
    if let Err(e) = download_file(&download_url, &archive_path, proxy) {
        let _ = fs::remove_file(&archive_path);
        return Err(e);
    }

    let extract_tmp = parent.join(format!(".tmp_extract_{semver}"));
    if extract_tmp.exists() {
        fs::remove_dir_all(&extract_tmp)?;
    }
    fs::create_dir_all(&extract_tmp)?;
    if let Err(e) = extract_archive(&archive_path, &extract_tmp) {
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&extract_tmp);
        return Err(e);
    }

    let _ = fs::remove_file(&archive_path);

    let jdk_home = match locate_jdk_home(&extract_tmp) {
        Ok(home) => home,
        Err(e) => {
            let _ = fs::remove_dir_all(&extract_tmp);
            return Err(e);
        }
    };

    if jdk_home.parent().unwrap() != parent {
        if let Err(e) = fs::rename(&jdk_home, &install_dir) {
            let _ = fs::remove_dir_all(&extract_tmp);
            return Err(e.into());
        }
        let _ = fs::remove_dir_all(&extract_tmp);
    } else if install_dir != jdk_home {
        if let Err(e) = fs::rename(&jdk_home, &install_dir) {
            let _ = fs::remove_dir_all(&extract_tmp);
            return Err(e.into());
        }
    }

    let full_version = jdk::detect_version(&install_dir)
        .with_context(|| format!("cannot detect JDK version in {}", install_dir.display()))?;

    let mut aliases = jdk::generate_aliases(&full_version);
    for alias in &opts.aliases {
        if !aliases.contains(alias) {
            aliases.push(alias.clone());
        }
    }

    let info = JdkInfo {
        path: install_dir.to_string_lossy().to_string(),
        full_version: full_version.clone(),
        aliases,
    };

    let mut config = config::Config::load()?;
    config.add_or_update_jdk(&info)?;
    config.save()?;

    println!("Installed JDK {full_version} ({})", install_dir.display());
    Ok(())
}

fn resolve_version(
    opts: &JdkInstallOpts,
    proxy: Option<&str>,
) -> Result<(String, String, String, Option<i64>)> {
    let client = build_client(proxy)?;
    let version_input = opts.version.as_deref().unwrap_or_default();

    let major: i32 = match version_input.find('.') {
        Some(_) => version_input
            .split('.')
            .next()
            .unwrap()
            .parse()
            .with_context(|| format!("invalid version: {version_input}"))?,
        None => version_input
            .parse::<i32>()
            .with_context(|| format!("invalid version: {version_input}"))?,
    };

    let url = format!(
        "{}/assets/feature_releases/{major}/ga?architecture={}&image_type=jdk&os={}&project=jdk&vendor={}&sort_method=DEFAULT&sort_order=DESC&page=0&page_size=5",
        opts.dist.api_base(),
        current_arch(),
        current_os(),
        opts.dist.vendor(),
    );

    let resp = client.get(&url).send()?;
    let releases: Vec<AdoptiumRelease> = resp.json()?;

    if releases.is_empty() {
        bail!(
            "no JDK {major} releases found for {}/{}",
            current_os(),
            current_arch()
        );
    }

    let release = if version_input.contains('.') {
        releases
            .iter()
            .find(|r| {
                r.version_data.semver == version_input
                    || r.version_data.openjdk_version.as_deref() == Some(version_input)
            })
            .unwrap_or(&releases[0])
    } else {
        &releases[0]
    };

    let semver = release.version_data.semver.clone();
    let bin = &release.binaries[0];
    Ok((
        semver,
        bin.pkg.link.clone(),
        bin.pkg.name.clone(),
        bin.pkg.size,
    ))
}

fn download_file(url: &str, dest: &Path, proxy: Option<&str>) -> Result<()> {
    let client = build_client(proxy)?;
    let mut resp = client.get(url).send()?;
    let total = resp.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
            .progress_chars("=>-"),
    );
    pb.set_message(format!(
        "Downloading {}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));

    let mut file = fs::File::create(dest)?;
    let mut downloaded = 0u64;
    let mut buf = [0u8; 65536];

    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();
    Ok(())
}

fn extract_archive(archive_path: &Path, dest: &Path) -> Result<()> {
    let name = archive_path.to_string_lossy().to_lowercase();

    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = fs::File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest)?;
    } else if name.ends_with(".zip") {
        let file = fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(dest)?;
    } else {
        bail!("unsupported archive format: {}", archive_path.display());
    }

    Ok(())
}

fn locate_jdk_home(dir: &Path) -> Result<PathBuf> {
    if jdk::java_bin_path(dir).exists() {
        return Ok(dir.to_path_buf());
    }

    let subdirs: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();

    for d in &subdirs {
        if jdk::java_bin_path(d).exists() {
            return Ok(d.to_path_buf());
        }
    }

    for d in &subdirs {
        if let Ok(entries) = fs::read_dir(d) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && jdk::java_bin_path(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    bail!("cannot locate JDK directory in {}", dir.display());
}

fn format_size(bytes: i64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else {
        format!("{} B", bytes as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dist_source_parse() {
        assert!("temurin".parse::<DistSource>().is_ok());
        assert!("Temurin".parse::<DistSource>().is_ok());
        assert!("TEMURIN".parse::<DistSource>().is_ok());
        assert!("corretto".parse::<DistSource>().is_err());
    }

    #[test]
    fn test_resolve_proxy_cli_first() {
        let result = resolve_proxy(Some("http://cli:8080"));
        assert_eq!(result, Some("http://cli:8080".to_string()));
    }

    #[test]
    fn test_resolve_proxy_env_fallback() {
        env::set_var("HTTPS_PROXY", "http://env:3128");
        let result = resolve_proxy(None);
        assert_eq!(result, Some("http://env:3128".to_string()));
        env::remove_var("HTTPS_PROXY");
    }

    #[test]
    fn test_resolve_proxy_adds_scheme() {
        let result = resolve_proxy(Some("192.168.1.1:8080"));
        assert_eq!(result, Some("http://192.168.1.1:8080".to_string()));
    }

    #[test]
    fn test_resolve_proxy_empty() {
        env::set_var("HTTPS_PROXY", "");
        let result = resolve_proxy(None);
        assert!(result.is_none());
        env::remove_var("HTTPS_PROXY");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_current_os_arch() {
        let os = current_os();
        assert!(["linux", "mac", "windows"].contains(&os));
        let arch = current_arch();
        assert!(["x64", "aarch64", "arm"].contains(&arch));
    }
}
