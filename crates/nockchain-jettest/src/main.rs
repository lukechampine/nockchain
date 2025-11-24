use core::iter::once;
use std::hint::spin_loop;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use flume::Receiver;
use futures::stream::iter;
use futures::{Stream, StreamExt};
use itertools::Itertools;
use nbx_jetpack::engine::Engine;
#[cfg(feature = "gpu")]
use nbx_jetpack::gpu;
use nbx_jetpack::{bpoly_to_fpoly, nbx_jets, new_fpoly, snag_as_poly_mary};
use nockapp::kernel::boot::{self, Cli};
use nockapp::kernel::form::SerfThread;
use nockapp::noun::slab::NounSlab;
use nockapp::save::SaveableCheckpoint;
use nockapp::utils::{NOCK_STACK_1KB, NOCK_STACK_SIZE_TINY};
use nockapp::wire::Wire;
use nockapp::NounExt;
use nockvm::jets::hot::{HotEntry, URBIT_HOT_STATE};
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::{D, T};
use nockvm_macros::tas;
use zkvm_jetpack::hot::produce_prover_hot_state;

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
    GenerateProof(GenerateProof),
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
        use std::collections::BTreeMap;
        use std::time::Instant;

        use anyhow::anyhow;
        use nbx_jetpack::codewords::CodewordEngine;
        use nbx_jetpack::substitute::SubstituteEngine;
        use nbx_jetpack::{compute_table_polys, mp_substitute_ultra_impl};
        use nockvm::jets::util::slot;
        use zkvm_jetpack::form::belt::*;
        use zkvm_jetpack::form::felt::*;
        use zkvm_jetpack::form::mary::*;
        use zkvm_jetpack::form::melt::*;
        use zkvm_jetpack::form::noun_ext::NounMathExt;
        use zkvm_jetpack::form::poly::*;
        use zkvm_jetpack::form::structs::{HoonList, HoonMapIter};

        gpu::GpuRegistry::builder()
            .add_gpu(None, 0, gpu::DEFAULT_GPU_QUEUE_SIZE)
            .unwrap()
            .build()
            .unwrap();

        match self {
            Self::Hash => Ok(nbx_jetpack::gpu::gpu_test().unwrap()),
            Self::Sub { mpsub_sam } => {
                let subject = load_jam(mpsub_sam)?;
                let subject = *unsafe { subject.root() };

                let inp = slot(subject, 6).unwrap();

                let [p, trace_evals, height, chals, dyns] = inp.uncell()?;

                let Ok(trace_evals) = BPolySlice::try_from(trace_evals) else {
                    return Err(anyhow!("Can't parse trace_evals"));
                };
                let trace_evals: BPolyVec = PolyVec(trace_evals.0.into());
                let trace_evals: MPolyVec = trace_evals.into();

                let height = height.as_atom()?.as_u64()?;
                let Ok(chals) = BPolySlice::try_from(chals) else {
                    return Err(anyhow!("Can't parse chals"));
                };

                let Ok(dyns) = BPolySlice::try_from(dyns) else {
                    return Err(anyhow!("Can't parse dyns"));
                };

                let mut engine = SubstituteEngine::new(height);
                mp_substitute_ultra_impl::<Melt>(
                    &mut engine,
                    0,
                    p,
                    (&trace_evals).into(),
                    chals,
                    dyns,
                )
                .unwrap();
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
                // compute-table-polys
                let table_polys_vec = compute_table_polys(&table_marys_vec);
                let table_polys = table_polys_vec
                    .iter()
                    .map(MarySlice::from)
                    .collect::<Vec<_>>();
                let engine = CodewordEngine::new(table_polys, fri_domain_len, total_cols);
                let t = Instant::now();
                let (codeword_array, height, mh) =
                    engine.clone().reduce_gpu(gpu::get_available_gpu().unwrap());
                println!("{}", codeword_array.dat.len());
                println!("{:?} {:?}", &codeword_array.dat[..10], mh.h);
                std::fs::write("gpu.txt", format!("{:#?}", &codeword_array.dat)).ok();
                println!(
                    "GPU: {:.02} {height} | {} {}",
                    t.elapsed().as_secs_f32(),
                    codeword_array.step,
                    codeword_array.len
                );
                let (codeword_array, height, mh) = engine.clone().reduce_cpu();
                println!("{:?} {:?}", &codeword_array.dat[..10], mh.h);
                println!(
                    "CPU: {:.02} {height} | {} {}",
                    t.elapsed().as_secs_f32(),
                    codeword_array.step,
                    codeword_array.len
                );
                std::fs::write("cpu.txt", format!("{:#?}", &codeword_array.dat)).ok();

                Ok(())
            }
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct GenerateProof {
    #[arg(long, default_value = "8", help = "pow-len used in the prover input")]
    pow_len: u64,
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
    #[arg(
        short = 'I',
        long,
        help = "Target GPU ID to use in tests (post-filtering)",
        default_value = "0"
    )]
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

        #[cfg(feature = "gpu")]
        if use_gpu {
            gpu::GpuRegistry::builder()
                .add_gpu(gpu_filter.as_deref(), gpu_id, gpu::DEFAULT_GPU_QUEUE_SIZE)
                .unwrap()
                .build()
                .unwrap();
        }

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
                let res = on_kernel(src_event.clone(), final_hot_state, cli.clone()).await?;
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

impl GenerateProof {
    async fn run(self, cli: Cli) -> Result<()> {
        let Self { pow_len } = self;

        let exp_hash = match pow_len {
            8 => (74351, 1616099586),
            16 => (86860, 232615266),
            64 => (115235, 1111470563),
            _ => panic!("unhandled pow_len: {pow_len}"),
        };

        let candidate = {
            let mut slab = NounSlab::new();
            let header = T(&mut slab, &[1, 2, 3, 4, 5].map(D));
            let nonce = T(&mut slab, &[6, 7, 8, 9, 10].map(D));
            // Very permissive target so the proof-of-work check always succeeds.
            let mut target = [u32::MAX as u64; 14];
            target[0] = tas!(b"bn");
            target[13] = 0;
            let target = T(&mut slab, &target.map(D));
            let cause = T(&mut slab, &[D(2), header, nonce, target, D(pow_len)]);
            slab.set_root(cause);
            slab
        };

        let mut jetted_hot = Vec::new();
        jetted_hot.extend(nbx_jets());
        jetted_hot.extend(URBIT_HOT_STATE);
        jetted_hot.extend(produce_prover_hot_state());

        let t0 = tokio::time::Instant::now();
        let jet_effect = on_kernel(candidate, jetted_hot, cli).await?;
        let elapsed = t0.elapsed();
        let jet_hash = hash_slab(&jet_effect);

        if jet_hash != exp_hash {
            anyhow::bail!("generate-proof test FAILED: {jet_hash:?} != {exp_hash:?}");
        }
        println!("OK (pow_len = {pow_len}) in {:.02}s", elapsed.as_secs_f64());
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
    let serf = SerfThread::<SaveableCheckpoint, rayon::ThreadPool>::new(
        kernels::miner::KERNEL.into(),
        None,
        hot_state,
        NOCK_STACK_SIZE_TINY,
        vec![],
        cli.trace_opts.into(),
        false,
    )
    .await
    .expect("Could not load mining kernel");

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

async fn on_kernel(slab: NounSlab, hot_state: Vec<HotEntry>, cli: Cli) -> Result<NounSlab> {
    let effects = run_kernel(iter(once(SendSlab(slab))), hot_state, cli).await;
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

    rayon::ThreadPoolBuilder::default().build_global().unwrap();
    #[cfg(feature = "threaded")]
    for _ in 0..rayon::current_num_threads() {
        rayon::spawn(|| loop {
            rayon::yield_now();
            spin_loop();
        });
    }

    match cli.mode {
        Mode::Test(p) => p.run(cli.nockapp_cli).await,
        Mode::GenerateProof(p) => p.run(cli.nockapp_cli).await,
        #[cfg(feature = "gpu")]
        Mode::GpuTest(p) => p.run(cli.nockapp_cli).await,
    }
}
