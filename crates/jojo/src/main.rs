use anyhow::Result;
use clap::{Parser, Subcommand};
use core::iter::once;
use either::Either;
use flume::Receiver;
use futures::Stream;
use futures::{stream::iter, StreamExt};
use nockapp::kernel::boot::{self, Cli};
use nockapp::kernel::checkpoint::JamPaths;
use nockapp::kernel::form::Kernel;
use nockapp::utils::{create_context, NOCK_STACK_SIZE, NOCK_STACK_SIZE_HUGE};
use nockapp::wire::Wire;
use nockapp::{noun::slab::NounSlab, Noun, NounExt};
use nockvm::interpreter::{Context, Error as IntError, Mote, Slogger};
use nockvm::jets::cold::{Cold, Nounable};
use nockvm::jets::hot::URBIT_HOT_STATE;
use nockvm::jets::nock::util::mook;
use nockvm::jets::util::slot;
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::{Cell, DirectAtom, IndirectAtom, D, T};
use nockvm::serialization::{cue, jam};
use nockvm::trace::path_to_cord;
use nockvm::unifying_equality::unifying_equality;
use nockvm_macros::tas;
use std::path::Path;
use std::pin::Pin;
use tempfile::tempdir;
use tokio::fs;
use tracing::{debug, info};
use zkvm_jetpack::hot::produce_prover_hot_state;

mod parse;
mod shell;
use shell::Shell;

struct SendSlab(NounSlab);

unsafe impl Send for SendSlab {}
unsafe impl Sync for SendSlab {}

pub static KERNEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/jojo.jam"
));

pub enum JojoWire {
    Run,
}

impl JojoWire {
    pub fn verb(&self) -> &'static str {
        match self {
            JojoWire::Run => "run",
        }
    }
}

impl Wire for JojoWire {
    const VERSION: u64 = 1;
    const SOURCE: &'static str = "jojo";

    fn to_wire(&self) -> nockapp::wire::WireRepr {
        let tags = vec![self.verb().into()];
        nockapp::wire::WireRepr::new(JojoWire::SOURCE, JojoWire::VERSION, tags)
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Mode {
    Jettest(Jettest),
    #[command(subcommand)]
    Eval(Eval),
    Shell,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Eval {
    Hoon {
        #[arg(help = "hoon string to evaluate")]
        hoon: String,
    },
    WithSubject {
        #[arg(help = "name of the function to evaluate")]
        func: String,
        #[arg(help = "path to the jammed subject containing sample")]
        subject: String,
    },
    WithSample {
        #[arg(help = "name of the function to evaluate")]
        func: String,
        #[arg(help = "path to the jammed sample")]
        sample: String,
    },
}

impl Eval {
    async fn run(self, cli: Cli) -> Result<()> {
        match self {
            Self::Hoon { hoon } => {
                let mut slab = NounSlab::new();
                let hoon =
                    unsafe { IndirectAtom::new_raw_bytes(&mut slab, hoon.len(), hoon.as_ptr()) };
                let poke = T(&mut slab, &[D(tas!(b"raw")), D(0), hoon.as_noun(), D(0)]);
                slab.set_root(poke);
                on_kernel(slab, cli).await
            }
            Self::WithSubject { func, subject } => with_jam(func, subject, 6, cli).await,
            Self::WithSample { func, sample } => with_jam(func, sample, 1, cli).await,
        }
    }
}

async fn with_jam(func: String, path: String, axis: u64, cli: Cli) -> Result<()> {
    let jam = std::fs::read(path)?;
    let mut slab = NounSlab::new();
    let noun = slab.cue_into(jam.into())?;
    let func = unsafe { IndirectAtom::new_raw_bytes(&mut slab, func.len(), func.as_ptr()) };
    let poke = T(
        &mut slab,
        &[
            D(tas!(b"sam")),
            D(0),
            func.as_noun(),
            D(0),
            slot(noun, axis).unwrap(),
        ],
    );
    slab.set_root(poke);
    on_kernel(slab, cli).await
}

#[derive(Parser, Debug, Clone)]
pub struct Jettest {
    #[arg(short, long, default_value = ".")]
    jamdir: String,
    #[arg(short, long)]
    interpret: bool,
    #[arg(short, long)]
    jet_run: bool,
    #[arg(long)]
    snapshot_dir: Option<String>,
    #[arg(long)]
    cold_jam: Option<String>,
    #[arg(long)]
    dump_dir: Option<String>,
}

impl Jettest {
    async fn run(self, cli: Cli) -> Result<()> {
        let Self {
            jamdir,
            interpret,
            jet_run,
            snapshot_dir,
            cold_jam,
            dump_dir,
        } = self;

        let hot_state = produce_prover_hot_state();
        let hot_state = [URBIT_HOT_STATE, &hot_state].concat();

        let mut stack = NockStack::new(NOCK_STACK_SIZE, 0);

        let p = Path::new(&jamdir);
        let subject = load_jam(&mut stack, p.join("subject.jam"))?;
        let formula = load_jam(&mut stack, p.join("formula.jam"))?;
        let jetpath = load_jam(&mut stack, p.join("jetpath.jam"));
        let cold = load_jam(
            &mut stack,
            if let Some(p) = cold_jam.as_deref() {
                Path::new(p).into()
            } else {
                p.join("cold.jam")
            },
        );

        let cold = if let Some(snapshot_dir) = snapshot_dir {
            let jam_paths = JamPaths::new(Path::new(&snapshot_dir));
            let checkpoint = if jam_paths.checkpoint_exists() {
                info!("Found existing state - restoring from checkpoint");
                jam_paths.load_checkpoint(&mut stack).ok()
            } else {
                info!("No existing state found");
                None
            };

            let (cold, event_num_raw) = checkpoint.as_ref().map_or_else(
                || (Cold::new(&mut stack), 0),
                |snapshot| (snapshot.cold, snapshot.event_num),
            );

            debug!("Cold state from event {event_num_raw}");

            cold
        } else if let Ok(cold) = cold {
            let cold = Cold::from_noun(&mut stack, &cold)?;
            Cold::from_vecs(&mut stack, cold.0, cold.1, cold.2)
        } else {
            debug!("Snapshot dir unspecified, no jammed cold state, making empty cold state!");
            Cold::new(&mut stack)
        };

        let mut context = create_context(stack, &hot_state, cold, cli.trace_opts.clone().into());

        let mut jet_res = None;
        if jet_run {
            let jetpath = jetpath?;
            let jetcord = path_to_cord(&mut context.stack, jetpath);
            let jetcord = std::str::from_utf8(jetcord.as_ne_bytes()).unwrap_or("");
            debug!("Formula in question: {jetcord}");
            let jetpath = jetpath.as_cell()?;

            for (path, _, jet) in hot_state {
                let mut a_path = D(0);
                for i in path {
                    match i {
                        Either::Left(tas) => {
                            let chum = unsafe {
                                IndirectAtom::new_raw_bytes_ref(&mut context.stack, tas)
                                    .normalize_as_atom()
                            }
                            .as_noun();
                            a_path = T(&mut context.stack, &[chum, a_path]);
                        }
                        Either::Right((tas, ver)) => {
                            let chum = T(
                                &mut context.stack,
                                &[
                                    DirectAtom::new_panic(*tas).as_atom().as_noun(),
                                    DirectAtom::new_panic(*ver).as_atom().as_noun(),
                                ],
                            );
                            a_path = T(&mut context.stack, &[chum, a_path]);
                        }
                    };
                }

                if unsafe {
                    unifying_equality(&mut context.stack, &mut jetpath.as_noun(), &mut a_path)
                } {
                    eprintln!("Found Jet!");
                    match jet(&mut context, subject) {
                        Ok(res) => {
                            let m = mug(&mut context.stack, res);
                            jet_res = Some((res, m));
                            eprintln!("Jet Ran OK (result mug: {m:?})");
                        }
                        Err(e) => {
                            eprintln!("ERROR JET: {e:?}");
                        }
                    }
                }
            }
        }

        let mut int_res = None;
        if interpret {
            match nockvm::interpreter::interpret(&mut context, subject, formula) {
                Ok(res) => {
                    let m = mug(&mut context.stack, res);
                    int_res = Some((res, m));
                    eprintln!("Ran OK (result mug: {m:?})");
                }
                Err(IntError::Deterministic(a, e)) => {
                    eprintln!("ERROR INTERPRETING:");
                    println!("{a:?} | {e:?}");
                    let goof = goof(&mut context, a, e);
                    print_goof(
                        &mut context.stack,
                        unsafe { Pin::get_unchecked_mut(context.slogger.as_mut()) },
                        goof,
                    );
                }
                Err(e) => {
                    eprintln!("ERROR INTERPRETING: {e:?}");
                }
            }
        }

        if let Some(dir) = dump_dir {
            for ((res, _mug), name) in [jet_res.zip(Some("jet.jam")), int_res.zip(Some("int.jam"))]
                .into_iter()
                .flatten()
            {
                fs::create_dir_all(&dir).await?;
                let atom = jam(&mut context.stack, res);
                fs::write(Path::new(&dir).join(name), atom.as_ne_bytes()).await?;
            }
        }

        Ok(())
    }
}

/// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "jojo")]
pub struct JojoCli {
    #[command(flatten)]
    pub nockapp_cli: Cli,
    #[command(subcommand)]
    pub mode: Mode,
}

fn load_jam(stack: &mut NockStack, path: impl AsRef<Path>) -> Result<Noun> {
    let jam = std::fs::read(path)?;
    let jam = unsafe { IndirectAtom::new_raw_bytes(stack, jam.len(), jam.as_ptr()) };
    let cue = cue(stack, jam.as_atom()).unwrap();
    Ok(cue)
}

async fn run_kernel(
    pokes: impl Stream<Item = SendSlab> + Send + 'static,
    cli: Cli,
) -> Receiver<SendSlab> {
    let snapshot_dir =
        tokio::task::spawn_blocking(|| tempdir().expect("Failed to create temporary directory"))
            .await
            .expect("Failed to create temporary directory");
    let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
    let snapshot_path_buf = snapshot_dir.path().to_path_buf();
    let jam_paths = JamPaths::new(snapshot_dir.path());

    let kernel = Kernel::load_with_hot_state(
        snapshot_path_buf,
        jam_paths,
        KERNEL,
        &hot_state,
        cli.trace_opts.into(),
    )
    .await
    .expect("Could not load jojo kernel");

    let (tx, rx) = flume::bounded(0);

    let task = async move {
        let mut pokes = core::pin::pin!(pokes);
        while let Some(slab) = pokes.next().await {
            let effects = kernel
                .poke(JojoWire::Run.to_wire(), slab.0)
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

async fn on_kernel(slab: NounSlab, cli: Cli) -> Result<()> {
    let effects = run_kernel(iter(once(SendSlab(slab))), cli).await;
    let effects_slab = effects.recv_async().await.unwrap().0;

    for effect in effects_slab.to_vec() {
        let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
            drop(effect);
            continue;
        };
        let eff = effect_cell.head().as_direct()?;
        if eff.data() != tas!(b"jojo") {
            return Err(anyhow::anyhow!("Unknown effect type: {eff:?}"));
        }
        let effect_cell = effect_cell.tail().as_cell()?;
        let raw = effect_cell.head().as_cell()?;
        println!("Raw {raw:?}");
        match effect_cell.tail().as_either_atom_cell() {
            Either::Left(_) => (),
            Either::Right(c) => {
                let pretty = c.tail().as_atom()?;
                let pretty = pretty.as_ne_bytes();
                let pretty = std::str::from_utf8(pretty)?.trim_end_matches('\0');
                println!("Pretty {pretty}");
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    nockvm::check_endian();
    let cli = JojoCli::parse();
    boot::init_default_tracing(&cli.nockapp_cli);

    match cli.mode {
        Mode::Jettest(j) => j.run(cli.nockapp_cli).await,
        Mode::Eval(e) => e.run(cli.nockapp_cli).await,
        Mode::Shell => Shell::default().run(cli.nockapp_cli).await,
    }
}

pub fn goof(context: &mut Context, mote: Mote, traces: Noun) -> Noun {
    let tone = Cell::new(&mut context.stack, D(2), traces);
    let tang = mook(context, tone, false)
        .expect("serf: goof: +mook crashed on bail")
        .tail();
    T(&mut context.stack, &[D(mote as u64), tang])
}

pub fn print_goof(stack: &mut NockStack, slogger: &mut (impl Slogger + ?Sized), goof: Noun) {
    let tang = goof
        .as_cell()
        .expect("print goof: expected goof to be a cell")
        .tail();
    tang.list_iter().for_each(|tank: Noun| {
        println!("TANK {tank:?}");
        //  TODO: Slogger should be emitting Results in case of failure
        slogger.slog(stack, 1, tank);
    });
}
