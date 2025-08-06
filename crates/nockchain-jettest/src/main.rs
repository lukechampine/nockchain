use anyhow::Result;
use clap::{Parser, Subcommand};
use nockapp::save::SaveableCheckpoint;
use core::iter::once;
use flume::Receiver;
use futures::Stream;
use futures::{stream::iter, StreamExt};
use itertools::Itertools;
use nockapp::kernel::boot::{self, Cli};
use nockapp::kernel::form::{Kernel, SerfThread};
use nockapp::utils::{NOCK_STACK_1KB, NOCK_STACK_SIZE_TINY};
use nockapp::wire::Wire;
use nockapp::{noun::slab::NounSlab, NounExt};
use nockvm::jets::hot::HotEntry;
use nockvm::jets::hot::URBIT_HOT_STATE;
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use std::path::Path;
use std::time::Instant;
use zkvm_jetpack::hot::produce_prover_hot_state;
use nbx_jetpack::nbx_jets;
#[cfg(feature = "gpu")]
use nbx_jetpack::gpu;

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
    #[cfg(feature = "gpu")]
    #[command(subcommand)]
    GpuTest(GpuTest),
}

#[cfg(feature = "gpu")]
#[derive(Subcommand, Debug, Clone)]
pub enum GpuTest {
    Hash,
    Sub {
        #[arg(help = "path to mp_substitute_ultra subject.jam")]
        mpsub_sam: String,
    },
    Codewords {
        #[arg(help = "path to mp_substitute_ultra subject.jam")]
        codeword_sam: String,
    },
}

#[cfg(feature = "gpu")]
impl GpuTest {
    async fn run(self, _: Cli) -> Result<()> {
        use anyhow::anyhow;
        use nockvm::jets::util::slot;
        use zkvm_jetpack::form::{BPolySlice, BPolyVec, Belt, MPolyVec, Melt, PolyVec, mary::{Mary, MarySlice}};
        use zkvm_jetpack::hand::structs::{HoonMapIter, HoonList};
        use nbx_jetpack::{substitute::SubstituteEngine, codewords::CodewordEngine, compute_table_polys, mp_substitute_ultra_impl};
        use zkvm_jetpack::noun::noun_ext::NounExt as ZNounExt;
        use std::collections::BTreeMap;
        use std::time::Instant;

        nbx_jetpack::gpu::init_gpu(None, 0);

        match self {
            Self::Hash => Ok(nbx_jetpack::gpu::gpu_test().unwrap()),
            Self::Sub { mpsub_sam } => {
                let subject = load_jam(mpsub_sam)?;
                let subject = *unsafe { subject.root() };

                let inp = slot(subject, 6).unwrap();

                let [p, trace_evals, height, chal_map, dyns] = inp.uncell()?;

                let Ok(trace_evals) = BPolySlice::try_from(trace_evals) else {
                    return Err(anyhow!("Can't parse trace_evals"));
                };
                let trace_evals: BPolyVec = PolyVec(trace_evals.0.into());
                let trace_evals: MPolyVec = trace_evals.into();

                let height = height.as_atom()?.as_u64()?;
                let chal_map = HoonMapIter::try_from(chal_map).ok().into_iter().flatten().map(|v| {
                    let [k, v] = v.uncell().unwrap().map(|v| v.as_atom().unwrap().as_u64().unwrap());
                    (k, Belt(v))
                }).collect::<BTreeMap<_, _>>();

                let Ok(dyns) = BPolySlice::try_from(dyns) else {
                    return Err(anyhow!("Can't parse dyns"));
                };

                let mut engine = SubstituteEngine::new(height);
                mp_substitute_ultra_impl::<Melt>(
                    &mut engine,
                    0,
                    p,
                    (&trace_evals).into(),
                    &chal_map,
                    dyns,
                ).unwrap();
                nbx_jetpack::gpu::gpu_sub_test(engine).unwrap();

                Ok(())
            }
            Self::Codewords { codeword_sam } => {
                let subject = load_jam(codeword_sam)?;
                let subject = *unsafe { subject.root() };

                let sam = slot(subject, 6).unwrap();

                let [table_marys, fri_domain_len, total_cols] = sam.uncell()?;
                let mut table_marys_vec = vec![];
                for m in HoonList::try_from(table_marys).ok().into_iter().flatten() {
                    let ma = MarySlice::try_from(m).unwrap();
                    table_marys_vec.push(ma);
                }
                let fri_domain_len = fri_domain_len.as_atom()?.as_u64()? as u32;
                let total_cols = total_cols.as_atom()?.as_u64()?;
                // ^-  codeword-commitments
                // ::
                // ::  convert the ext columns to marys
                // ::
                // ::  think of each mary as a list of the table's columns, interpolated to polynomials
                // =/  table-polys=(list mary)
                //   (compute-table-polys table-marys)
                let table_polys_vec = compute_table_polys(&table_marys_vec);
                let table_polys = table_polys_vec.iter().map(MarySlice::from).collect::<Vec<_>>();
                let engine = CodewordEngine::new(table_polys, fri_domain_len, total_cols);
                let t = Instant::now();
                let (codeword_array, height, mh) = engine.clone().reduce_gpu();
                println!("{}", codeword_array.dat.len());
                println!("{:?} {:?}", &codeword_array.dat[..10], mh.h);
                std::fs::write("gpu.txt", format!("{:#?}", &codeword_array.dat));
                println!("GPU: {:.02} {height} | {} {}", t.elapsed().as_secs_f32(), codeword_array.step, codeword_array.len);
                let (codeword_array, height, mh) = engine.clone().reduce_cpu();
                println!("{:?} {:?}", &codeword_array.dat[..10], mh.h);
                println!("CPU: {:.02} {height} | {} {}", t.elapsed().as_secs_f32(), codeword_array.step, codeword_array.len);
                std::fs::write("cpu.txt", format!("{:#?}", &codeword_array.dat));

                Ok(())
            }
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct Test {
    #[arg(short, long)]
    src_event: String,
    #[arg(short, long, help = "effect to compare jetted results against")]
    effect: Option<String>,
    #[arg(long, help = "where to write effect if it mismatches")]
    error_out: Option<String>,
    #[arg(short, long, help = "permute through jet combinations")]
    permute: bool,
    #[arg(
        short,
        long,
        help = "maximum number of jets to disable when permuting",
        requires = "permute"
    )]
    max_disable: Option<usize>,
    #[cfg(feature = "gpu")]
    #[arg(short = 'g', long, help = "Use GPU when testing?")]
    use_gpu: bool,
    #[cfg(feature = "gpu")]
    #[arg(short = 'F', long, help = "Target GPU to filter against")]
    gpu_filter: Option<String>,
    #[cfg(feature = "gpu")]
    #[arg(short = 'I', long, help = "Target GPU ID to use in tests (post-filtering)", default_value = "0")]
    gpu_id: usize,
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
            error_out,
            permute,
            max_disable,
            #[cfg(feature = "gpu")]
            use_gpu,
            #[cfg(feature = "gpu")]
            gpu_filter,
            #[cfg(feature = "gpu")]
            gpu_id,
        } = self;

        let init_call = move || {
            #[cfg(feature = "gpu")]
            if use_gpu {
                gpu::init_gpu(gpu_filter.as_deref(), gpu_id);
            }
        };
        let init_call = Some(init_call);

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
                let mut final_hot_state = vec![];
                final_hot_state.extend(comb.iter().map(|i| permute_jets[*i]));
                final_hot_state.extend(hot_state.iter().cloned());

                println!("Testing combination:");

                for (i, e) in enabled.into_iter().enumerate() {
                    println!("{} - {}", if e { "ON " } else { "OFF" }, jet_names[i]);
                }

                let time = Instant::now();
                let res = on_kernel(src_event.clone(), final_hot_state, cli.clone(), init_call.clone()).await?;
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

                if res_hash != src_effect_hash && error_out.is_some() {
                    let loc = error_out.as_ref().unwrap();
                    tokio::fs::write(loc, res.jam()).await?;
                    println!("Wrote got effect to {loc}");
                }
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
    init_call: Option<impl FnOnce() + Send + 'static>
) -> Receiver<SendSlab> {

    let serf = SerfThread::<SaveableCheckpoint>::new(
        kernels::miner::KERNEL.into(),
        None,
        hot_state,
        NOCK_STACK_SIZE_TINY,
        vec![],
        cli.trace_opts.into(),
    )
    .await
    .expect("Could not load mining kernel");

    if let Some(init_call) = init_call {
        serf.call_fn(init_call).await.unwrap();
    }

    let (tx, rx) = flume::bounded(0);

    let task = async move {
        let mut pokes = core::pin::pin!(pokes);
        while let Some(slab) = pokes.next().await {
            let effects = serf
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

async fn on_kernel(slab: NounSlab, hot_state: Vec<HotEntry>, cli: Cli, init_call: Option<impl FnOnce() + Send + 'static>) -> Result<NounSlab> {
    let effects = run_kernel(iter(once(SendSlab(slab))), hot_state, cli, init_call).await;
    let effects_slab = effects.recv_async().await.unwrap().0;

    for effect in effects_slab.to_vec() {
        let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
            drop(effect);
            continue;
        };

        if effect_cell.head().eq_bytes("mine-result") {
            return Ok(effects_slab);
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
        #[cfg(feature = "gpu")]
        Mode::GpuTest(p) => p.run(cli.nockapp_cli).await,
    }
}
