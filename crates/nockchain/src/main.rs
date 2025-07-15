use std::error::Error;

use clap::Parser;
use kernels::dumb::KERNEL;
use nockapp::kernel::boot;
use nockapp::NockApp;
use zkvm_jetpack::hot::produce_prover_hot_state;
use nbx_jetpack::nbx_jets;
use jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    nockvm::check_endian();
    let cli = nockchain::NockchainCli::parse();
    boot::init_default_tracing(&cli.nockapp_cli);

    let mut prover_hot_state = nbx_jets().collect::<Vec<_>>();
    prover_hot_state.extend(produce_prover_hot_state());
    let mut nockchain: NockApp =
        nockchain::init_with_kernel(Some(cli), KERNEL, prover_hot_state.as_slice()).await?;
    nockchain.run().await?;
    Ok(())
}
