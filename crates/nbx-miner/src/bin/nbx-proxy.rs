use clap::Parser;
use nbx_miner::proxy::ProxyConfig;
use nockapp::kernel::boot::{self, Cli as NockappCli};
use serde::{Deserialize, Serialize};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Serialize, Deserialize)]
pub struct ProxyCfg {
    proxy: ProxyConfig,
    #[cfg(feature = "prom-exporter")]
    prometheus_bind: String,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "nbx-miner")]
pub struct ProxyCli {
    #[command(flatten)]
    proxy: ProxyConfig,
    #[command(flatten)]
    nockapp_cli: NockappCli,
    #[cfg(feature = "prom-exporter")]
    #[arg(long, default_value = "127.0.0.1:9006")]
    pub prometheus_bind: String,
    #[arg(long, help = "Path to config")]
    pub config: Option<String>,
    #[arg(long, help = "Print current config and exit")]
    pub print_config: bool,
}

#[tokio::main]
async fn main() {
    let mut cli = ProxyCli::parse();
    if let Some(config) = cli.config.take() {
        let config: ProxyCfg = toml::from_str(
            &tokio::fs::read_to_string(config)
                .await
                .expect("Config file not found"),
        )
        .expect("Unable to parse config");
        cli.proxy = config.proxy;
        #[cfg(feature = "prom-exporter")]
        {
            cli.prometheus_bind = config.prometheus_bind;
        }
        cli.update_from(std::env::args_os());
    }

    let config = ProxyCfg {
        proxy: cli.proxy,
        #[cfg(feature = "prom-exporter")]
        prometheus_bind: cli.prometheus_bind,
    };

    if cli.print_config {
        println!("{}", toml::to_string_pretty(&config).unwrap());
        return;
    }

    #[cfg(feature = "prom-exporter")]
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(
            config
                .prometheus_bind
                .parse::<std::net::SocketAddr>()
                .expect("Invalid socket address"),
        )
        .idle_timeout(
            metrics_util::MetricKindMask::ALL,
            Some(std::time::Duration::from_secs(300)),
        )
        .install()
        .expect("Unable to install prometheus exporter");

    nockvm::check_endian();
    #[cfg(not(feature = "stealthy"))]
    boot::init_default_tracing(&cli.nockapp_cli);
    nbx_miner::proxy::run_proxy(config.proxy).await;
}
