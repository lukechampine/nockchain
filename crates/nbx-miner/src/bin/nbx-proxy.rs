use clap::Parser;
use nbx_miner::proxy::ProxyConfig;
use nockapp::kernel::boot::{self, Cli as NockappCli};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

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
}

#[tokio::main]
async fn main() {
    let cli = ProxyCli::parse();

    #[cfg(feature = "prom-exporter")]
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(
            cli.prometheus_bind
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
    nbx_miner::proxy::run_proxy(cli.proxy).await;
}
