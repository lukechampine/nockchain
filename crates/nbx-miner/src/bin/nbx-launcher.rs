use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use nbx_miner::device::runtime_cpu_level;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const API: &'static str = "https://pool-api.nockbox.org";

#[derive(Clone, Copy, Debug, ValueEnum, Serialize, Deserialize, PartialEq)]
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
#[command(color = clap::ColorChoice::Auto)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Start {
        #[command(subcommand)]
        settings: Settings,
    },
    // Restart using the existing configuration
    Restart {
        program: Program,
    },
}

#[derive(Debug, Subcommand)]
enum Settings {
    /// Run as miner
    #[command(subcommand)]
    Miner(MinerCommands),
    /// Run as proxy server
    Proxy {
        #[command(flatten)]
        pool_opts: PoolOptions,

        #[command(flatten)]
        common_opts: CommonOptions,

        #[arg(
            last = true,
            help = "Additional arguments to forward to the proxy (after --, e.g. --prometheus-bind 0.0.0.0:9000. Use --help for help)"
        )]
        forward: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum MinerCommands {
    Direct {
        #[command(flatten)]
        pool_opts: PoolOptions,

        #[command(flatten)]
        common_opts: CommonOptions,

        #[arg(
            last = true,
            help = "Additional arguments to forward to miner (after --, e.g. --num-threads 4. Use --help for help)"
        )]
        forward: Vec<String>,
    },
    Proxy {
        /// Required proxy URL to connect through
        proxy_url: String,

        #[command(flatten)]
        common_opts: CommonOptions,

        #[arg(
            last = true,
            help = "Additional arguments to forward to miner (after --, e.g. --num-threads 4. Use --help for help)"
        )]
        forward: Vec<String>,
    },
}

impl Settings {
    fn program(&self) -> Program {
        match self {
            Self::Miner { .. } => Program::Miner,
            Self::Proxy { .. } => Program::Proxy,
        }
    }

    fn miner_connect(&self) -> &String {
        match self {
            Self::Miner(MinerCommands::Direct { pool_opts, .. }) => &pool_opts.pool_url,
            Self::Miner(MinerCommands::Proxy { proxy_url, .. }) => &proxy_url,
            Self::Proxy { pool_opts, .. } => &pool_opts.pool_url,
        }
    }

    fn needs_token(&self) -> bool {
        match self {
            Self::Miner(MinerCommands::Direct { pool_opts, .. }) => true,
            Self::Miner(MinerCommands::Proxy { .. }) => false,
            Self::Proxy { pool_opts, .. } => true,
        }
    }

    fn auth_token(&self) -> Option<&str> {
        match self {
            Self::Miner(MinerCommands::Direct { pool_opts, .. }) => pool_opts.auth.as_deref(),
            Self::Miner(MinerCommands::Proxy { .. }) => None,
            Self::Proxy { pool_opts, .. } => pool_opts.auth.as_deref(),
        }
    }

    fn common_opts(&self) -> &CommonOptions {
        match self {
            Self::Miner(MinerCommands::Direct { common_opts, .. }) => common_opts,
            Self::Miner(MinerCommands::Proxy { common_opts, .. }) => common_opts,
            Self::Proxy { common_opts, .. } => common_opts,
        }
    }

    fn forward_args(&self) -> &Vec<String> {
        match self {
            Self::Miner(MinerCommands::Direct { forward, .. }) => forward,
            Self::Miner(MinerCommands::Proxy { forward, .. }) => forward,
            Self::Proxy { forward, .. } => forward,
        }
    }
}

#[derive(Debug, Args)]
struct PoolOptions {
    /// Pool URL to connect to
    #[arg(long = "pool", default_value = "pool-proxy.nockbox.org:4344")]
    pool_url: String,

    /// Authentication token (JWT)
    #[arg(long)]
    auth: Option<String>,
}

#[derive(Debug, Args, Serialize, Deserialize, PartialEq, Clone)]
struct CommonOptions {
    /// Overwrite existing config
    #[arg(long)]
    force_overwrite: bool,

    #[arg(long)]
    client_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct LocalConfig {
    miner_connect: String,
    program: Program,
    needs_token: bool,
    access_token: String,
    cpu_level: String,

    common_options: CommonOptions,
    forward_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct BinaryResponse {
    version: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { settings } => start(settings).await?,
        Commands::Restart { program } => restart(program).await?,
    };

    Ok(())
}

async fn refresh_token(access_token: &str) -> Result<TokenResponse> {
    reqwest::Client::new()
        .post(format!("{API}/api/v1/credentials/refresh"))
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()
        .context("Failed to refresh token")?
        .json()
        .await
        .context("Failed to parse refresh token response")
}

async fn setup_token(access_token: &str) -> Result<TokenResponse> {
    reqwest::Client::new()
        .post(format!("{API}/api/v1/credentials/setup"))
        .bearer_auth(&access_token)
        .send()
        .await?
        .error_for_status()
        .context("Failed to exchange token")?
        .json()
        .await
        .context("Failed to parse token response")
}

async fn refresh_or_create_config(settings: &Settings, cfg_path: &Path) -> Result<LocalConfig> {
    let existing_config: Result<LocalConfig> = read_toml(cfg_path).await;

    let access_token = if let Ok(existing_config) = existing_config {
        println!("Refreshing authentication token...");
        let response = refresh_token(&existing_config.access_token).await?;
        response.token
    } else {
        let Some(auth_token) = settings.auth_token() else {
            eprintln!("The authentication token was not set (--auth). This is needed for binary updates and direct connections.");
            std::process::exit(1);
        };

        println!("Fetching authentication token...");
        let response = setup_token(auth_token).await?;
        response.token
    };

    let config = LocalConfig {
        program: settings.program(),
        miner_connect: settings.miner_connect().to_string(),
        needs_token: settings.needs_token(),
        access_token,
        cpu_level: runtime_cpu_level().to_string(),
        common_options: settings.common_opts().clone(),
        forward_args: settings.forward_args().clone(),
    };

    write_toml(&cfg_path, &config).await?;

    Ok(config)
}

async fn fetch_latest_release(program: Program, config: &LocalConfig) -> Result<BinaryResponse> {
    let response = reqwest::Client::new()
        .post(format!(
            "{API}/api/v1/releases/{}/latest",
            match program {
                Program::Miner => format!(
                    "nbx-miner-{}",
                    config.cpu_level.strip_prefix("x86_64-").unwrap_or("v2")
                ),
                Program::Proxy => "nbx-proxy".to_string(),
            },
        ))
        .bearer_auth(&config.access_token)
        .send()
        .await?;

    // Check status and include error body if failed
    if !response.status().is_success() {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());
        bail!("Request failed with status {}: {}", status, error_body);
    }

    response
        .json()
        .await
        .context("Failed to parse release response")
}

async fn restart(program: Program) -> Result<()> {
    let cfg_path = config_file_path(program)?;
    ensure_parent_dir(&cfg_path).await?;

    let Ok(mut existing_config) = read_toml::<LocalConfig>(&cfg_path).await else {
        eprintln!("Existing configuration not found");
        std::process::exit(1);
    };

    // Step 1: Fetch a fresh access token
    println!("Refreshing authentication token...");
    let response = refresh_token(&existing_config.access_token).await?;
    existing_config.access_token = response.token;
    write_toml(&cfg_path, &existing_config).await?;

    execute(existing_config).await?;

    Ok(())
}

async fn start(settings: Settings) -> Result<()> {
    let cfg_path = config_file_path(settings.program())?;
    ensure_parent_dir(&cfg_path).await?;

    // Preparation: Remove the existing configuration when requested
    if settings.common_opts().force_overwrite {
        if cfg_path.exists() {
            println!("Removing existing configuration...");
            fs::remove_file(&cfg_path).await?;
        }
    }

    // Step 1: Fetch a fresh access token
    let config = refresh_or_create_config(&settings, &cfg_path).await?;

    execute(config).await?;

    Ok(())
}

async fn execute(config: LocalConfig) -> Result<()> {
    // Step 2: Request latest binary info from backend
    println!("Checking for latest binary version...");
    let latest_release = fetch_latest_release(config.program, &config).await?;

    // Step 3: Download the latest binary if it's different from the existing binary
    let cache_bin = cache_bin_path(config.program)?;
    let cache_version = cache_version_path(config.program)?;

    ensure_parent_dir(&cache_bin).await?;
    ensure_parent_dir(&cache_version).await?;

    let cached_version = fs::read_to_string(&cache_version).await.ok();

    if let Some(cached_version) = cached_version.as_ref() {
        println!("Current installed version {}", cached_version);
    }

    let target_release = format!("{} {}", latest_release.version.trim(), config.cpu_level);

    let need_download =
        cached_version.is_none_or(|cached_version| cached_version.trim() != target_release);

    if need_download {
        println!(
            "Downloading {} '{}' from '{}'",
            config.program.as_str(),
            latest_release.version,
            latest_release.url,
        );

        let bytes = reqwest::Client::new()
            .get(&latest_release.url)
            .send()
            .await?
            .error_for_status()
            .context("Failed to download binary")?
            .bytes()
            .await
            .context("Failed to read binary data")?;

        // Remove version file
        let _ = fs::remove_file(&cache_version).await;

        // Write binary
        {
            let mut f = fs::File::create(&cache_bin).await?;
            f.write_all(&bytes).await?;
            f.flush().await?;
        }

        // Make it executable
        let perms = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(&cache_bin, perms).await?;

        // Write version file
        let mut f = fs::File::create(&cache_version).await?;
        f.write_all(target_release.as_bytes()).await?;
        f.flush().await?;

        println!(
            "Installed {} {} to {}",
            config.program.as_str(),
            latest_release.version,
            cache_bin.display()
        );
    }

    // Step 4: Launch the binary
    println!("Starting {}...", config.program.as_str());

    let mut args = vec!["--miner-connect".to_string(), config.miner_connect];

    let mut envs = vec![];

    if config.needs_token {
        envs.push(("NBX_AUTH_JWT", config.access_token));
    }

    if let Some(client_name) = config.common_options.client_name.as_ref() {
        args.extend(["--client-name".to_string(), client_name.to_string()]);
    };

    let status = Command::new(&cache_bin)
        .args(args)
        .args(&config.forward_args)
        .envs(envs)
        .status()
        .await
        .with_context(|| format!("failed to launch {}", cache_bin.display()))?;

    if !status.success() {
        bail!("Process exited with {}", status);
    }

    Ok(())
}

async fn write_toml<T: Serialize>(path: &Path, val: &T) -> Result<()> {
    let toml = toml::to_string_pretty(val)?;
    let mut f = fs::File::create(path).await?;
    f.write_all(toml.as_bytes()).await?;
    f.flush().await?;
    Ok(())
}

async fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let contents = fs::read_to_string(path).await?;
    let val = toml::from_str(&contents)?;
    Ok(val)
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

fn cache_version_path(program: Program) -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no cache dir"))?
        .join("nbx");
    Ok(dir.join(format!("{}.version", program.as_str())))
}

async fn ensure_parent_dir(p: &Path) -> Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).await?;
    }
    Ok(())
}
