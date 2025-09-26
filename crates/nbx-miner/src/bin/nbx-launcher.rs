/*use clap::{ColorChoice, Parser};
use clap_serde_derive::ClapSerde;
use nbx_miner::client_base::ClientConfig;
use serde::{Deserialize, Serialize};
use dirs::config_dir;

// When enabled, use jemalloc for more stable memory allocation
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser)]
pub struct LauncherCli {
    temp_token: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = LauncherCli::parse();
}*/

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const API: &'static str = "https://pool-api.nockbox.org";

#[derive(Clone, Copy, Debug, ValueEnum, Serialize, Deserialize)]
enum Program {
    Miner,
    Proxy,
}

impl Program {
    fn as_str(self) -> &'static str {
        match self {
            Program::Miner => "miner",
            Program::Proxy => "proxy",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, Serialize, Deserialize)]
enum Target {
    Direct,
    Proxy,
}

#[derive(Debug, Parser)]
#[command(color=clap::ColorChoice::Auto)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Generate config for a NockBox miner or proxy
    GenerateConfig {
        #[arg(value_enum)]
        program: Program,

        /// Required when program=miner
        #[arg(value_enum, required_if_eq("program", "miner"))]
        target: Option<Target>,

        /// Overwrite existing config
        #[arg(long)]
        force_overwrite: bool,
    },

    /// Ensure latest binary in cache and launch it with the config
    Launch {
        #[arg(value_enum)]
        program: Program,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct MinerConfig {
    program: Program,
    target: Target,
    #[serde(skip_serializing_if = "Option::is_none")]
    jwt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProxyConfig {
    program: Program,
    jwt: String,
}

#[derive(Debug, Deserialize)]
struct JwtResp {
    token: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseResp {
    version: String,
    sha256: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::GenerateConfig {
            program,
            target,
            force_overwrite,
        } => generate_config(program, target, force_overwrite).await?,
        Cmd::Launch { program } => launch(program).await?,
    }
    Ok(())
}

async fn generate_config(program: Program, target: Option<Target>, force: bool) -> Result<()> {
    let cfg_path = config_file_path(program)?;
    ensure_parent_dir(&cfg_path).await?;

    if fs::try_exists(&cfg_path).await? && !force {
        bail!(
            "Config already exists at {}. Use --force-overwrite to replace.",
            cfg_path.display()
        );
    }

    match program {
        Program::Miner => {
            let tgt = target.expect("clap enforces required_if");
            let jwt = match tgt {
                Target::Direct => Some(fetch_jwt().await?),
                Target::Proxy => None,
            };
            let cfg = MinerConfig {
                program,
                target: tgt,
                jwt,
            };
            write_toml(&cfg_path, &cfg).await?;
            println!("Wrote miner config → {}", cfg_path.display());
        }
        Program::Proxy => {
            let jwt = fetch_jwt().await?;
            let cfg = ProxyConfig { program, jwt };
            write_toml(&cfg_path, &cfg).await?;
            println!("Wrote proxy config → {}", cfg_path.display());
        }
    }
    Ok(())
}

async fn launch(program: Program) -> Result<()> {
    // 1) Ensure config exists
    let cfg_path = config_file_path(program)?;
    if !fs::try_exists(&cfg_path).await? {
        bail!(
            "Missing config at {}. Run `generate-config` first.",
            cfg_path.display()
        );
    }

    // 2) Get release metadata
    let rel: ReleaseResp = reqwest::Client::new()
        .get(format!("{API}/release"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("parsing /release")?;

    let cache_bin = cache_bin_path(program)?;
    let cache_release = cache_release_meta_path(program)?;
    ensure_parent_dir(&cache_bin).await?;
    ensure_parent_dir(&cache_release).await?;

    // 3) Check current version we have
    let need_download = match fs::read_to_string(&cache_release).await.ok() {
        Some(s) => !s.lines().next().is_some_and(|v| v.trim() == rel.version),
        None => true,
    };

    if need_download {
        let url = format!("{API}/v{}/linux/{}", rel.version, program.as_str());
        let bytes = reqwest::Client::new()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await
            .context("downloading binary")?;

        // Store release metadata (version + sha)
        let mut f = fs::File::create(&cache_release).await?;
        f.write_all(format!("{}\n{}\n", rel.version, rel.sha256).as_bytes())
            .await?;
        f.flush().await?;

        // Write binary
        {
            let mut f = fs::File::create(&cache_bin).await?;
            f.write_all(&bytes).await?;
            f.flush().await?;
        }
        // Make it executable (Tokio set_permissions; create perms via PermissionsExt)
        let perms = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(&cache_bin, perms).await?;

        println!(
            "Installed {} v{} to {}",
            program.as_str(),
            rel.version,
            cache_bin.display()
        );
    } else {
        println!(
            "Latest {} is already cached at {}",
            program.as_str(),
            cache_bin.display()
        );
    }

    // 4) Exec binary with --config <...>
    let status = Command::new(&cache_bin)
        .arg("--config")
        .arg(&cfg_path)
        .status()
        .await
        .with_context(|| format!("failed to launch {}", cache_bin.display()))?;

    if !status.success() {
        bail!("Process exited with {}", status);
    }
    Ok(())
}

async fn fetch_jwt() -> Result<String> {
    let JwtResp { token } = reqwest::Client::new()
        .post(format!("{API}/API/v1/generate-jwt"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("parsing jwt")?;
    Ok(token)
}

async fn write_toml<T: Serialize>(path: &Path, val: &T) -> Result<()> {
    let toml = toml::to_string_pretty(val)?;
    let mut f = fs::File::create(path).await?;
    f.write_all(toml.as_bytes()).await?;
    f.flush().await?;
    Ok(())
}

fn config_file_path(program: Program) -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no config dir"))?
        .join("nbx");
    Ok(dir.join(format!("{}.toml", program.as_str())))
}

fn cache_bin_path(program: Program) -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no cache dir"))?
        .join("nbx");
    Ok(dir.join(program.as_str()))
}

fn cache_release_meta_path(program: Program) -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no cache dir"))?
        .join("nbx");
    Ok(dir.join(format!("{}.release", program.as_str())))
}

async fn ensure_parent_dir(p: &Path) -> Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).await?;
    }
    Ok(())
}
