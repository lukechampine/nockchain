use clap::Parser;
use nbx_miner::client::ClientConfig;
use nockapp::kernel::boot::{self, Cli as NockappCli};

// When enabled, use jemalloc for more stable memory allocation
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(Parser, Debug, Clone)]
#[command(name = "nbx-miner")]
pub struct Cli {
    #[command(flatten)]
    client: ClientConfig,
    #[command(flatten)]
    nockapp_cli: NockappCli,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    nockvm::check_endian();
    #[cfg(not(feature = "stealthy"))]
    boot::init_default_tracing(&cli.nockapp_cli);
    nbx_miner::client::run_client(cli.client).await;
}
