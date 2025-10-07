use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use jsonwebtoken::{DecodingKey, Validation};
use nbx_miner::device::{runtime_cpu_level, RANDOMNESS_ENV};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const API: &'static str = "https://pool-api.nockbox.org";
const DEFAULT_CONNECT: &'static str = "pool-proxy.nockbox.org:4344";

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
        common_opts: CommonOptions,

        #[arg(
            help = "Socket to bind the proxy on for downstream miners to connect to",
            default_value = "[::]:4344"
        )]
        bind_addr: String,

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

        // Not used as the binary can be download without a token, but here for backwards compatibility
        #[arg(long, hide(true))]
        auth: Option<String>,

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

    fn miner_connect(&self) -> &str {
        match self {
            Self::Miner(MinerCommands::Direct { .. }) => DEFAULT_CONNECT,
            Self::Miner(MinerCommands::Proxy { proxy_url, .. }) => &proxy_url,
            Self::Proxy { .. } => DEFAULT_CONNECT,
        }
    }

    fn miner_bind(&self) -> Option<&str> {
        match self {
            Self::Miner(_) => None,
            Self::Proxy { bind_addr, .. } => Some(&bind_addr),
        }
    }

    fn needs_token(&self) -> bool {
        match self {
            Self::Miner(MinerCommands::Direct { .. }) => true,
            Self::Miner(MinerCommands::Proxy { .. }) => false,
            Self::Proxy { .. } => true,
        }
    }

    fn auth_token(&self) -> Option<&str> {
        match self {
            Self::Miner(MinerCommands::Direct { common_opts, .. }) => common_opts.auth.as_deref(),
            Self::Miner(MinerCommands::Proxy { auth, .. }) => auth.as_deref(),
            Self::Proxy { common_opts, .. } => common_opts.auth.as_deref(),
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
struct ProxyOptions {
    /// Pool URL to connect to
    #[arg(long = "pool", default_value = "pool-proxy.nockbox.org:4344")]
    proxy_url: String,
}

#[derive(Debug, Args, Serialize, Deserialize, PartialEq, Clone)]
struct CommonOptions {
    /// Authentication token (JWT)
    #[arg(long)]
    auth: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct SharedConfig {
    access_token: Option<String>,
    randomness: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct LocalConfig {
    miner_connect: String,
    #[serde(default)]
    miner_bind: Option<String>,
    program: Program,
    needs_token: bool,
    forward_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct Claims {
    iat: u64,
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
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_aud = false;
    validation.validate_nbf = false;
    validation.validate_exp = false;

    match jsonwebtoken::decode::<Claims>(access_token, &DecodingKey::from_secret(&[]), &validation)
    {
        Ok(c) if jsonwebtoken::get_current_timestamp().saturating_sub(c.claims.iat) < 60 => {
            return Ok(TokenResponse {
                token: access_token.into(),
            });
        }
        _ => {
            println!("Refreshing authentication token...");
        }
    }

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

async fn refresh_or_create_config(settings: &Settings) -> Result<SharedConfig> {
    let cfg_path = shared_config_file_path()?;
    ensure_parent_dir(&cfg_path).await?;
    let existing_config: Result<SharedConfig> = read_toml(&cfg_path).await;

    let access_token = if let Some(auth_token) = settings.auth_token() {
        println!("Fetching new authentication token (remove --auth to skip)");
        let response = setup_token(auth_token).await?;
        Some(response.token)
    } else if let Ok(SharedConfig {
        access_token: Some(ref access_token),
        ..
    }) = existing_config
    {
        let response = refresh_token(access_token).await?;
        Some(response.token)
    } else if !settings.needs_token() {
        None
    } else {
        eprintln!("The authentication token was not set (--auth), and no previous token saved. This is needed for direct connections.");
        std::process::exit(1);
    };

    let config = if let Ok(cfg) = existing_config {
        SharedConfig {
            access_token,
            ..cfg
        }
    } else {
        SharedConfig {
            access_token,
            randomness: rand::random::<u64>().to_string(),
        }
    };

    write_toml(&cfg_path, &config).await?;

    Ok(config)
}

async fn create_config(settings: &Settings) -> Result<LocalConfig> {
    let cfg_path = config_file_path(settings.program())?;
    ensure_parent_dir(&cfg_path).await?;

    let config = LocalConfig {
        program: settings.program(),
        miner_connect: settings.miner_connect().to_string(),
        miner_bind: settings.miner_bind().map(|v| v.to_string()),
        needs_token: settings.needs_token(),
        forward_args: settings.forward_args().clone(),
    };

    write_toml(&cfg_path, &config).await?;

    Ok(config)
}

async fn fetch_latest_release(program: Program) -> Result<BinaryResponse> {
    let response = reqwest::Client::new()
        .post(format!(
            "{API}/api/v1/releases/{}/latest",
            match program {
                Program::Miner => format!(
                    "nbx-miner-{}",
                    runtime_cpu_level().strip_prefix("x86_64-").unwrap_or("v2")
                ),
                Program::Proxy => "nbx-proxy".to_string(),
            },
        ))
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
    let shared_cfg_path = shared_config_file_path()?;
    ensure_parent_dir(&cfg_path).await?;

    let Ok(local_config) = read_toml::<LocalConfig>(&cfg_path).await else {
        eprintln!("Existing {} configuration not found", program.as_str());
        std::process::exit(1);
    };

    let Ok(mut shared_config) = read_toml::<SharedConfig>(&shared_cfg_path).await else {
        eprintln!("Existing shared configuration not found");
        std::process::exit(1);
    };

    if local_config.needs_token {
        let Some(access_token) = shared_config.access_token.as_ref() else {
            eprintln!("The authentication token was not set");
            std::process::exit(1);
        };

        let response = refresh_token(access_token).await?;
        shared_config.access_token = Some(response.token);
        write_toml(&shared_cfg_path, &shared_config).await?;
    }

    execute(shared_config, local_config).await?;

    Ok(())
}

async fn start(settings: Settings) -> Result<()> {
    // Step 1: Fetch a fresh access token
    let shared_config = refresh_or_create_config(&settings).await?;
    let config = create_config(&settings).await?;

    execute(shared_config, config).await?;

    Ok(())
}

async fn execute(shared_config: SharedConfig, config: LocalConfig) -> Result<()> {
    // Step 2: Request latest binary info from backend
    println!("Checking for latest binary version...");
    let latest_release = fetch_latest_release(config.program).await?;

    // Step 3: Download the latest binary if it's different from the existing binary
    let cache_bin = cache_bin_path(config.program)?;
    let cache_version = cache_version_path(config.program)?;

    ensure_parent_dir(&cache_bin).await?;
    ensure_parent_dir(&cache_version).await?;

    let cached_version = fs::read_to_string(&cache_version).await.ok();

    if let Some(cached_version) = cached_version.as_ref() {
        println!("Current installed version {}", cached_version);
    }

    let target_release = format!("{} {}", latest_release.version.trim(), runtime_cpu_level());

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

    if let Some(bind) = config.miner_bind {
        args.extend(["--miner-bind".to_string(), bind]);
    }

    let mut envs = vec![(RANDOMNESS_ENV, shared_config.randomness)];

    if config.needs_token {
        let Some(access_token) = shared_config.access_token else {
            eprintln!("The authentication token was not set");
            std::process::exit(1);
        };

        envs.push(("NBX_AUTH_JWT", access_token));
    }

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

fn shared_config_file_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no config dir"))?
        .join("nbx");
    Ok(dir.join(format!("shared.toml")))
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
