use clap::{ColorChoice, Parser};
use clap_serde_derive::ClapSerde;
use nbx_miner::proxy::ProxyConfig;
use nbx_miner::server::MiningConfig;
use serde::{Deserialize, Serialize};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(ClapSerde, Deserialize, Serialize)]
pub struct ProxyCfg {
    #[clap_serde]
    #[command(flatten)]
    proxy: ProxyConfig,
    #[clap_serde]
    #[command(flatten)]
    server: MiningConfig,
    #[cfg(feature = "prom-exporter")]
    #[arg(long)]
    prometheus_bind: Option<String>,
}

#[derive(Parser)]
#[command(name = "nbx-miner")]
pub struct ProxyCli {
    #[command(flatten)]
    proxy: <ProxyCfg as ClapSerde>::Opt,
    #[arg(long, help = "Path to config")]
    pub config: Option<String>,
    #[arg(long, help = "Print current config and exit")]
    pub print_config: bool,
    #[arg(long, help = "Control colored output", value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

#[tokio::main]
async fn main() {
    let mut cli = ProxyCli::parse();
    let config = if let Some(config) = cli.config.take() {
        let config: <ProxyCfg as ClapSerde>::Opt = toml::from_str(
            &tokio::fs::read_to_string(config)
                .await
                .expect("Config file not found"),
        )
        .expect("Unable to parse config");
        ProxyCfg::from(config).merge(&mut cli.proxy)
    } else {
        ProxyCfg::from(cli.proxy)
    };

    if cli.print_config {
        println!("{}", toml::to_string_pretty(&config).unwrap());
        return;
    }

    #[cfg(feature = "prom-exporter")]
    if let Some(prometheus_bind) = config.prometheus_bind {
        metrics_exporter_prometheus::PrometheusBuilder::new()
            .with_http_listener(
                prometheus_bind
                    .parse::<std::net::SocketAddr>()
                    .map_err(|_| {
                        println!("Invalid socket address for the prometheus exporter");
                        std::process::exit(1);
                    })
                    .unwrap(),
            )
            .idle_timeout(
                metrics_util::MetricKindMask::ALL,
                Some(std::time::Duration::from_secs(300)),
            )
            .install()
            .map_err(|e| {
                println!("Unable to install prometheus exporter");
                std::process::exit(1);
            })
            .unwrap();
    }

    nockvm::check_endian();
    nbx_miner::init_default_tracing(cli.color);
    nbx_miner::proxy::run_proxy(config.proxy, config.server).await;
}
