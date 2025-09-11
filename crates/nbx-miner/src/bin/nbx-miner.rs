use clap::Parser;
use nbx_miner::client_base::ClientConfig;
use nockapp::kernel::boot::{self, Cli as NockappCli};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(all(feature = "jemalloc", not(feature = "tracing-heap")))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(feature = "tracing-heap")]
#[global_allocator]
static ALLOC: tracy_client::ProfiledAllocator<tikv_jemallocator::Jemalloc> =
    tracy_client::ProfiledAllocator::new(tikv_jemallocator::Jemalloc, 100);

#[derive(Parser, Debug, Clone)]
#[command(name = "nbx-miner")]
pub struct Cli {
    #[command(flatten)]
    client: ClientConfig,
    #[command(flatten)]
    nockapp_cli: NockappCli,
    #[cfg(feature = "prom-exporter")]
    #[arg(long, default_value = "127.0.0.1:9006")]
    pub prometheus_bind: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.client.client_name.is_none() {
        panic!("Client name must be set");
    }

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
    nbx_miner::client::run_client(cli.client).await;
}
