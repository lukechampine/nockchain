use clap::{ColorChoice, Parser};
use clap_serde_derive::ClapSerde;
use nbx_miner::client_base::ClientConfig;
use serde::{Deserialize, Serialize};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(all(feature = "jemalloc", not(feature = "tracing-heap")))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(feature = "tracing-heap")]
#[global_allocator]
static ALLOC: tracy_client::ProfiledAllocator<tikv_jemallocator::Jemalloc> =
    tracy_client::ProfiledAllocator::new(tikv_jemallocator::Jemalloc, 100);

#[derive(ClapSerde, Deserialize, Serialize)]
pub struct MinerCfg {
    #[clap_serde]
    #[command(flatten)]
    client: ClientConfig,
    #[cfg(feature = "prom-exporter")]
    #[arg(long)]
    prometheus_bind: Option<String>,
}

#[derive(Parser)]
#[command(name = "nbx-miner")]
pub struct MinerCli {
    #[command(flatten)]
    miner: <MinerCfg as ClapSerde>::Opt,
    #[arg(long, help = "Path to config")]
    pub config: Option<String>,
    #[arg(long, help = "Print current config and exit")]
    pub print_config: bool,
    #[arg(long, help = "Control colored output", value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

#[tokio::main]
async fn main() {
    let mut cli = MinerCli::parse();
    let config = if let Some(config) = cli.config.take() {
        let config: <MinerCfg as ClapSerde>::Opt = toml::from_str(
            &tokio::fs::read_to_string(config)
                .await
                .expect("Config file not found"),
        )
        .expect("Unable to parse config");
        MinerCfg::from(config).merge(&mut cli.miner)
    } else {
        MinerCfg::from(cli.miner)
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
    nbx_miner::client::run_client(config.client.into()).await;
}
