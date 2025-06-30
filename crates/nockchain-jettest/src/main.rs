use anyhow::Result;
use clap::{Parser, Subcommand};
use nockapp::save::SaveableCheckpoint;
use core::iter::once;
use flume::Receiver;
use futures::Stream;
use futures::{stream::iter, StreamExt};
use itertools::Itertools;
use nockapp::kernel::boot::{self, Cli};
use nockapp::kernel::form::Kernel;
use nockapp::utils::NOCK_STACK_1KB;
use nockapp::wire::Wire;
use nockapp::{noun::slab::NounSlab, NounExt};
use nockvm::jets::hot::HotEntry;
use nockvm::jets::hot::URBIT_HOT_STATE;
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use std::path::Path;
use std::time::Instant;
use zkvm_jetpack::hot::produce_prover_hot_state;
use zkvm_jetpack::jets::nbx::nbx_jets;

pub enum MiningWire {
    Mined,
    Candidate,
    SetPubKey,
    Enable,
}

impl MiningWire {
    pub fn verb(&self) -> &'static str {
        match self {
            MiningWire::Mined => "mined",
            MiningWire::SetPubKey => "setpubkey",
            MiningWire::Candidate => "candidate",
            MiningWire::Enable => "enable",
        }
    }
}

impl Wire for MiningWire {
    const VERSION: u64 = 1;
    const SOURCE: &'static str = "miner";

    fn to_wire(&self) -> nockapp::wire::WireRepr {
        let tags = vec![self.verb().into()];
        nockapp::wire::WireRepr::new(MiningWire::SOURCE, MiningWire::VERSION, tags)
    }
}

struct SendSlab(NounSlab);

unsafe impl Send for SendSlab {}
unsafe impl Sync for SendSlab {}

#[derive(Subcommand, Debug, Clone)]
pub enum Mode {
    Test(Test),
}

#[derive(Parser, Debug, Clone)]
pub struct Test {
    #[arg(short, long)]
    src_event: String,
    #[arg(short, long, help = "effect to compare jetted results against")]
    effect: Option<String>,
    #[arg(short, long, help = "permute through jet combinations")]
    permute: bool,
    #[arg(
        short,
        long,
        help = "maximum number of jets to disable when permuting",
        requires = "permute"
    )]
    max_disable: Option<usize>,
}

fn hash_slab(s: &NounSlab) -> (usize, u64) {
    let mut stack = NockStack::new(NOCK_STACK_1KB, 0);
    let root = unsafe { s.root() };
    let m = mug(&mut stack, *root);
    let l = s.jam().len();
    (l, m.data())
}

impl Test {
    async fn run(self, cli: Cli) -> Result<()> {
        let Self {
            src_event,
            effect,
            permute,
            max_disable,
        } = self;

        let max_disable = if permute { max_disable } else { Some(0) };

        let hot_state = produce_prover_hot_state();
        let hot_state = [URBIT_HOT_STATE, &hot_state].concat();
        let permute_jets = nbx_jets().collect::<Vec<_>>();
        let jet_names = permute_jets
            .iter()
            .map(|(p, _, _)| std::str::from_utf8(p.last().unwrap().unwrap_left()).unwrap())
            .collect::<Vec<_>>();

        let src_event = load_jam(src_event)?;
        let src_event_hash = hash_slab(&src_event);
        println!("Loaded source event {src_event_hash:?}");

        let src_effect = load_jam(effect.unwrap())?;
        let src_effect_hash = hash_slab(&src_effect);
        println!("Loaded source effect {src_effect_hash:?}");

        for i in (0..=permute_jets.len())
            .rev()
            .take(max_disable.unwrap_or(permute_jets.len()) + 1)
        {
            for comb in (0..permute_jets.len()).combinations(i) {
                let mut enabled = vec![false; permute_jets.len()];
                comb.iter().for_each(|i| enabled[*i] = true);
                let mut hot_state = hot_state.clone();
                hot_state.extend(comb.iter().map(|i| permute_jets[*i]));

                println!("Testing combination:");

                for (i, e) in enabled.into_iter().enumerate() {
                    println!("{} - {}", if e { "ON " } else { "OFF" }, jet_names[i]);
                }

                let time = Instant::now();
                let res = on_kernel(src_event.clone(), hot_state, cli.clone()).await?;
                let res_hash = hash_slab(&res);

                println!(
                    "{} - Res effect {res_hash:?} in {:.02}s",
                    if res_hash == src_effect_hash {
                        "OK "
                    } else {
                        "ERR"
                    },
                    time.elapsed().as_secs_f64()
                );
            }
        }

        Ok(())
    }
}

/// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "jojo")]
pub struct JettestCli {
    #[command(flatten)]
    pub nockapp_cli: Cli,
    #[command(subcommand)]
    pub mode: Mode,
}

fn load_jam(path: impl AsRef<Path>) -> Result<NounSlab> {
    let jam = std::fs::read(path)?;
    let mut slab = NounSlab::new();
    let ret = slab.cue_into(jam.into())?;
    slab.set_root(ret);
    Ok(slab)
}

async fn run_kernel(
    pokes: impl Stream<Item = SendSlab> + Send + 'static,
    hot_state: Vec<HotEntry>,
    cli: Cli,
) -> Receiver<SendSlab> {

    let kernel = Kernel::<SaveableCheckpoint>::load_with_hot_state(
        kernels::miner::KERNEL,
        None,
        &hot_state,
        vec![],
        cli.trace_opts.into(),
    )
    .await
    .expect("Could not load jojo kernel");

    let (tx, rx) = flume::bounded(0);

    let task = async move {
        let mut pokes = core::pin::pin!(pokes);
        while let Some(slab) = pokes.next().await {
            let effects = kernel
                .poke(MiningWire::Candidate.to_wire(), slab.0)
                .await
                .expect("Could not poke jojo kernel with slab");

            if tx.send_async(SendSlab(effects)).await.is_err() {
                break;
            }
        }
    };

    tokio::spawn(task);

    rx
}

async fn on_kernel(slab: NounSlab, hot_state: Vec<HotEntry>, cli: Cli) -> Result<NounSlab> {
    let effects = run_kernel(iter(once(SendSlab(slab))), hot_state, cli).await;
    let effects_slab = effects.recv_async().await.unwrap().0;

    for effect in effects_slab.to_vec() {
        let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
            drop(effect);
            continue;
        };

        if effect_cell.head().eq_bytes("command") {
            return Ok(effect);
        }
    }

    Err(anyhow::anyhow!("No effect produced"))
}

#[tokio::main]
async fn main() -> Result<()> {
    nockvm::check_endian();
    let cli = JettestCli::parse();
    boot::init_default_tracing(&cli.nockapp_cli);

    match cli.mode {
        Mode::Test(p) => p.run(cli.nockapp_cli).await,
    }
}
