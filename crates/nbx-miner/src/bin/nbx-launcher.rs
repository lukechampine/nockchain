use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

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

#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
#[cfg(target_arch = "x86_64")]
const ARCH: &str = "x86_64";

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
    exp: u64,
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

async fn api_request<T: for<'de> Deserialize<'de>>(
    request: reqwest::RequestBuilder,
    context_msg: &str,
) -> Result<T> {
    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());
        bail!("{} ({}): {}", context_msg, status, error_body);
    }

    response
        .json()
        .await
        .with_context(|| format!("{}: Failed to parse response", context_msg))
}

async fn refresh_token(access_token: &str) -> Result<TokenResponse> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_aud = false;
    validation.validate_nbf = false;
    validation.validate_exp = false;

    let current_timestamp = jsonwebtoken::get_current_timestamp();

    let decoded =
        jsonwebtoken::decode::<Claims>(access_token, &DecodingKey::from_secret(&[]), &validation);

    // If token is recently issued, return it immediately
    if let Ok(ref c) = decoded {
        if current_timestamp.saturating_sub(c.claims.iat) < 5 * 60 {
            return Ok(TokenResponse {
                token: access_token.into(),
            });
        }
    }

    println!("Refreshing authentication token...");

    match api_request(
        reqwest::Client::new()
            .post(format!("{API}/api/v1/credentials/refresh"))
            .bearer_auth(access_token),
        "Failed to refresh token",
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(e) => match decoded {
            Ok(c) if c.claims.exp.saturating_sub(current_timestamp) >= 86400 => {
                println!("Refresh failed but existing token still valid, using existing token");
                Ok(TokenResponse {
                    token: access_token.into(),
                })
            }
            _ => Err(e),
        },
    }
}

async fn setup_token(access_token: &str) -> Result<TokenResponse> {
    api_request(
        reqwest::Client::new()
            .post(format!("{API}/api/v1/credentials/setup"))
            .bearer_auth(access_token),
        "Failed to exchange token",
    )
    .await
}

async fn refresh_or_create_config(settings: &Settings) -> Result<SharedConfig> {
    let cfg_path = shared_config_file_path()?;
    ensure_parent_dir(&cfg_path).await?;
    let existing_config: Result<SharedConfig> = read_toml(&cfg_path).await;

    let mut access_token = existing_config
        .as_ref()
        .ok()
        .and_then(|c| c.access_token.clone());

    if let Some(auth_token) = settings.auth_token() {
        println!("Fetching new authentication token (remove --auth to skip)");

        match setup_token(auth_token).await {
            Ok(response) => access_token = Some(response.token),
            Err(e) => {
                // Only error if this program requires a token and there is not already a local
                //  access token. Otherwise, the launcher should continue and refresh the existing
                //  token or proceed without access token at all.
                if settings.needs_token() && access_token.is_none() {
                    return Err(e);
                }

                eprintln!(
                    "Failed to fetch new authentication token, \
                    falling back to existing authentication token. {}",
                    e,
                );
            }
        }
    }

    if let Some(access_token) = access_token.as_mut() {
        *access_token = refresh_token(access_token).await?.token;
    }

    if settings.needs_token() && access_token.is_none() {
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
    api_request(
        reqwest::Client::new().post(format!(
            "{API}/api/v1/releases/{}/{ARCH}-linux/latest",
            match program {
                #[cfg(target_arch = "x86_64")]
                Program::Miner => format!(
                    "nbx-miner-{}",
                    runtime_cpu_level().strip_prefix("x86_64-").unwrap_or("v2")
                ),
                #[cfg(not(target_arch = "x86_64"))]
                Program::Miner => "nbx-miner".to_string(),
                Program::Proxy => "nbx-proxy".to_string(),
            },
        )),
        "Failed to fetch latest release",
    )
    .await
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
    let cache_bin = cache_bin_path(config.program)?;
    let cache_version = cache_version_path(config.program)?;

    ensure_parent_dir(&cache_bin).await?;
    ensure_parent_dir(&cache_version).await?;

    let cached_version = fs::read_to_string(&cache_version).await.ok();

    if let Some(cached_version) = cached_version.as_ref() {
        println!("Current installed version {}", cached_version);
    }

    // Step 2: Request latest binary info from backend
    println!("Checking for latest binary version...");
    let latest_release = match fetch_latest_release(config.program).await {
        Ok(release) => Some(release),
        Err(error) => {
            if cached_version.is_some() {
                println!(
                    "Failed to retrieve the latest binary version, continuing with the current version; {}",
                    error
                );
                None
            } else {
                return Err(error);
            }
        }
    };

    // Step 3: Download the latest binary if it's different from the existing binary
    if let Some(latest_release) = latest_release {
        let target_release = format!("{} {}", latest_release.version.trim(), runtime_cpu_level());

        let need_download = cached_version.as_ref().map_or(true, |cached_version| {
            cached_version.trim() != target_release
        });

        if need_download {
            println!(
                "Downloading {} '{}' from '{}'",
                config.program.as_str(),
                latest_release.version,
                latest_release.url,
            );

            // Download to binary to a temporary file first
            let temp_bin = cache_bin.with_file_name(format!(
                "{}.tmp",
                cache_bin.file_name().unwrap().to_string_lossy()
            ));

            let download_result = async {
                let bytes = reqwest::Client::new()
                    .get(&latest_release.url)
                    .send()
                    .await?
                    .error_for_status()
                    .context("Failed to download binary")?
                    .bytes()
                    .await
                    .context("Failed to read binary data")?;

                // Write to temporary binary file
                {
                    let mut f = fs::File::create(&temp_bin).await?;
                    f.write_all(&bytes).await?;
                    f.flush().await?;
                }

                // Make it executable
                let perms = std::fs::Permissions::from_mode(0o755);
                fs::set_permissions(&temp_bin, perms).await?;

                // Only now, replace the old binary
                fs::rename(&temp_bin, &cache_bin).await?;

                // Write to version file
                {
                    let mut f = fs::File::create(&cache_version).await?;
                    f.write_all(target_release.as_bytes()).await?;
                    f.flush().await?;
                }

                Ok(())
            }
            .await;

            match download_result {
                Ok(_) => {
                    println!(
                        "Installed {} {} to {}",
                        config.program.as_str(),
                        latest_release.version,
                        cache_bin.display()
                    );
                }
                Err(error) => {
                    // Clean up the temporary file
                    let _ = fs::remove_file(temp_bin).await;

                    if cached_version.is_some() {
                        println!(
                            "Failed to download new version, continuing with cached version; {}",
                            error
                        );
                    } else {
                        return Err(error);
                    }
                }
            }
        }
    }

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

    // Step 4: Launch the binary with auto-restart on crash
    loop {
        println!("Starting {}...", config.program.as_str());

        let start_time = Instant::now();

        let status = Command::new(&cache_bin)
            .args(&args)
            .args(&config.forward_args)
            .envs(envs.clone())
            .status()
            .await
            .with_context(|| format!("failed to launch {}", cache_bin.display()))?;

        let elapsed = start_time.elapsed();

        if status.success() {
            // Clean exit, break out of the loop
            break;
        }

        // If the process failed early, don't attempt to restart
        if elapsed.as_secs() < 60 {
            bail!("Process exited with {}", status);
        }

        eprintln!(
            "Process exited with {} after running for {} seconds. Auto-restarting...",
            status,
            elapsed.as_secs()
        );

        // Brief delay before restarting
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
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
