use std::collections::HashMap;
use std::path::Path;
use std::pin::Pin;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use nockapp::Bytes;
use core::iter::once;
use either::Either;
use flume::{Receiver, Sender};
use futures::Stream;
use futures::{stream::iter, StreamExt};
use nockapp::kernel::boot::{self, Cli};
use nockapp::kernel::checkpoint::JamPaths;
use nockapp::kernel::form::Kernel;
use nockapp::utils::{create_context, NOCK_STACK_SIZE_HUGE};
use nockapp::wire::Wire;
use nockapp::{noun::slab::NounSlab, Noun, NounExt};
use nockvm::interpreter::{Context, Error as IntError, Mote, Slogger};
use nockvm::jets::cold::{Cold, Nounable};
use nockvm::jets::hot::URBIT_HOT_STATE;
use nockvm::jets::nock::util::mook;
use nockvm::jets::util::slot;
use nockvm::jets::JetErr;
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::{Cell, DirectAtom, IndirectAtom, D, T};
use nockvm::serialization::cue;
use nockvm::trace::path_to_cord;
use nockvm::unifying_equality::unifying_equality;
use nockvm_macros::tas;
use reedline::{DefaultPrompt, DefaultPromptSegment, FileBackedHistory, Reedline, Signal};
use tempfile::tempdir;
use tracing::{debug, info};
use zkvm_jetpack::hot::produce_prover_hot_state;

mod parse;
use parse::{parse_tree, Node};

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
    Jojo(Jojo),
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
                let poke = T(&mut slab, &[D(tas!(b"raw")), hoon.as_noun()]);
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
        &[D(tas!(b"sam")), func.as_noun(), slot(noun, axis).unwrap()],
    );
    slab.set_root(poke);
    on_kernel(slab, cli).await
}

#[derive(Parser, Debug, Clone)]
pub struct Jojo {
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
}

impl Jojo {
    async fn run(self, cli: Cli) -> Result<()> {
        let Self {
            jamdir,
            interpret,
            jet_run,
            snapshot_dir,
            cold_jam,
        } = self;

        let hot_state = produce_prover_hot_state();
        let hot_state = [URBIT_HOT_STATE, &hot_state].concat();

        let mut stack = NockStack::new(NOCK_STACK_SIZE_HUGE, 0);

        let p = Path::new(&jamdir);
        let subject = load_jam(&mut stack, p.join("subject.jam"))?;
        let formula = load_jam(&mut stack, p.join("formula.jam"))?;
        let jetpath = load_jam(&mut stack, p.join("jetpath.jam"))?;
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

        let jetcord = path_to_cord(&mut context.stack, jetpath);
        let jetcord = std::str::from_utf8(jetcord.as_ne_bytes()).unwrap_or("");
        debug!("Formula in question: {jetcord}");
        let jetpath = jetpath.as_cell()?;

        let mut jet_res = None;
        if jet_run {
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

    let kernel = Kernel::load_with_hot_state_huge(
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
        let raw = effect_cell.head().as_cell()?;
        let pretty = effect_cell.tail().as_atom()?;
        let pretty = pretty.as_ne_bytes();
        let pretty = std::str::from_utf8(pretty)?.trim_end_matches('\0');
        println!("Raw {raw:?}");
        println!("Pretty {pretty}");
    }

    Ok(())
}

fn input_func(out: Sender<Option<String>>) -> Result<()> {
    loop {
        if out.send(None).is_err() {
            break;
        }

        let history = Box::new(FileBackedHistory::with_file(50, "jojo.hist".into())?);
        let mut rl = Reedline::create().with_history(history);

        let prompt = DefaultPrompt {
            left_prompt: DefaultPromptSegment::Basic("~nockvm:jojo".to_string()),
            right_prompt: DefaultPromptSegment::Empty,
        };

        match rl.read_line(&prompt)? {
            Signal::Success(line) => {
                if out.send(Some(line)).is_err() {
                    break;
                }
                let _ = rl.sync_history();
            }
            Signal::CtrlD | Signal::CtrlC => break,
        }
    }

    Ok(())
}

#[derive(Default)]
struct ShellCommand {
    hoon: Option<String>,
    in_sample: Option<Node>,
    out_sample: Option<String>,
    jam_out: Option<String>,
    cue_inp: Option<(String, usize)>,
}

impl ShellCommand {
    fn parse(mut line: &str) -> Result<Self> {
        let mut in_sample = None;
        let mut out_sample = None;
        let mut jam_out = None;
        let mut cue_inp = None;

        let hoon = if line.starts_with('/') {
            line = line.split_once('/').unwrap().1;

            loop {
                let (mut cmd, rest) = line.split_once(' ').unwrap_or((line, ""));

                let mut parse_sam = || {
                    let brack = rest.rfind(']');
                    let (cmd, mut rest) = brack
                        .map(|v| rest.split_at(v + 1))
                        .or_else(|| rest.split_once(' '))
                        .unwrap_or((rest, ""));

                    rest = rest.trim_ascii_start();

                    if cmd.is_empty() {
                        return Err(anyhow!("Sample is not specified"));
                    }

                    let Some(tree) = parse_tree(cmd) else {
                        return Err(anyhow!("Malformed sample: {cmd}"));
                    };

                    in_sample = Some(tree);

                    Ok((cmd, rest))
                };

                match cmd {
                    "h" | "help" => {
                        return Err(anyhow!(
                            r"Usage:

/h, /help - display this message.
/. sam func - evaluate `func` with the given sample `sam`. Given sample can be constructed from variable assignments (using /.), and can be a cell, e.g. [a b].
/= var hoon - evaluage `hoon` and assign output to `var`.
/p sam      - print given sample `sam`. Note: if single variable is provided, it is pretty-printed, but mutliple variables are printed as raw nouns.
/c var path - cue a jamfile at `path` and assign it to `var`.
/s var path - cue a subject jamfile at `path`, take the sample at axis 6, and assign it to `var`.
/j sam path - jam the given sample `sam` and write it to `path`.
hoon - evaluate `hoon` and print the result out to screen.
                            "
                        ))
                    }
                    "." => (_, line) = parse_sam()?,
                    "=" => {
                        (cmd, line) = rest.split_once(' ').unwrap_or((rest, ""));
                        if cmd.is_empty() {
                            return Err(anyhow!("Sample is not specified"));
                        }
                        out_sample = Some(cmd.to_string());
                    }
                    "p" => {
                        parse_sam()?;
                        break None;
                    }
                    "c" => {
                        let (var, path) = rest
                            .split_once(' ')
                            .ok_or_else(|| anyhow!("Unable to cue: invalid input"))?;
                        out_sample = Some(var.to_string());
                        cue_inp = Some((path.to_string(), 1));
                    }
                    "s" => {
                        let (var, path) = rest
                            .split_once(' ')
                            .ok_or_else(|| anyhow!("Unable to cue: invalid input"))?;
                        out_sample = Some(var.to_string());
                        cue_inp = Some((path.to_string(), 6));
                    }
                    "j" => {
                        (_, line) = parse_sam()?;
                        jam_out = Some(line.to_string());
                        break None;
                    }
                    _ => return Err(anyhow!("Invalid command! (use /help for usage)")),
                }

                break Some(line.trim_ascii_start());
            }
        } else {
            Some(line)
        };

        let hoon = hoon.map(|hoon| {
            if hoon.starts_with('(') || !hoon.contains(' ') {
                hoon.to_string()
            } else {
                format!("({hoon})")
            }
        });

        Ok(Self {
            hoon,
            in_sample,
            out_sample,
            jam_out,
            cue_inp,
        })
    }
}

#[derive(Default)]
struct ShellState {
    samples: HashMap<String, (bool, NounSlab)>,
    save_state: Option<String>,
}

enum Command {
    Eval(NounSlab),
    Jam(Bytes, String),
    Cue(String, usize),
}

impl ShellState {
    fn next_command(&mut self, line: &str) -> Result<Command> {
        let ShellCommand {
            hoon,
            in_sample,
            out_sample,
            jam_out,
            cue_inp,
        } = ShellCommand::parse(line)?;

        self.save_state = out_sample;

        let mut slab = NounSlab::new();

        let mut vased_sample = None;

        if let Some(tree) = in_sample {
            let pull_noun = |sample: String| {
                self.samples
                    .get(&sample)
                    .map(|(vased, v)| (vased, unsafe { *v.root() }))
                    .ok_or_else(|| anyhow!("{sample} is undefined"))
            };

            if let Node::Leaf(sample) = &tree {
                let (vased, mut noun) = pull_noun(sample.clone())?;
                if !vased {
                    noun = T(&mut slab, &[D(tas!(b"noun")), noun]);
                }
                vased_sample = Some(noun);
            } else {
                let noun = tree.fold(
                    &mut |sample: String| {
                        pull_noun(sample)
                            .map(|(vased, v)| if *vased { slot(v, 3).unwrap() } else { v })
                    },
                    &mut |nouns: Vec<Noun>| Ok(T(&mut slab, &nouns[..])),
                )?;
                vased_sample = Some(T(&mut slab, &[D(tas!(b"noun")), noun]));
            }
        }

        if let Some(jam) = jam_out {
            let sam = 
                vased_sample
                    .and_then(|v| slot(v, 3).ok())
                    .ok_or_else(|| anyhow!("Cannot jam without sample (impossible)"))?;
            let slab = NounSlab::from(sam);
            let bytes = slab.jam();
            return Ok(Command::Jam(
                bytes,
                jam,
            ));
        } else if let Some((cue, axis)) = cue_inp {
            return Ok(Command::Cue(cue, axis));
        }

        let hoon = hoon.as_deref().map(|v| unsafe {
            IndirectAtom::new_raw_bytes(&mut slab, v.len(), v.as_ptr()).as_noun()
        });

        let poke = match (hoon, vased_sample) {
            (Some(hoon), Some(sam)) => {
                let sam = slot(sam, 3).unwrap();
                slab.copy_into(sam);
                let sam = unsafe { *slab.root() };
                T(&mut slab, &[D(tas!(b"sam")), hoon, sam])
            }
            (Some(hoon), None) => T(&mut slab, &[D(tas!(b"raw")), hoon]),
            (None, Some(sam)) => {
                slab.copy_into(sam);
                let sam = unsafe { *slab.root() };
                T(&mut slab, &[D(tas!(b"prt")), sam])
            }
            _ => return Err(anyhow!("Invalid command")),
        };

        slab.set_root(poke);

        Ok(Command::Eval(slab))
    }

    /// Returns if should print
    fn process_out(&mut self, vased: bool, out: Noun) -> bool {
        if let Some(sam) = self.save_state.take() {
            self.samples.insert(sam, (vased, out.into()));
            false
        } else {
            true
        }
    }
}

async fn shell(cli: Cli) -> Result<()> {
    let (tx, rx) = flume::bounded(0);
    let effects = run_kernel(rx.into_stream(), cli).await;

    // Intentionally rendezvous!
    let (lines_out, lines) = flume::bounded(0);

    let task = tokio::task::spawn_blocking(move || input_func(lines_out));

    let mut shell = ShellState::default();

    while let Ok(line) = lines.recv_async().await {
        let Some(line) = line else { continue };

        if line.is_empty() {
            continue;
        }

        let command = match shell.next_command(&line) {
            Ok(s) => s,
            Err(e) => {
                println!("{e}");
                continue;
            }
        };

        match command {
            Command::Jam(bytes, path) => {
                if let Err(e) = tokio::fs::write(&path, bytes).await {
                    println!("Unable to save jam to {path}: {e}");
                }
            }
            Command::Cue(path, axis) => {
                let mut slab = NounSlab::new();
                let bytes = tokio::fs::read(path).await?;
                let noun = slab.cue_into(bytes.into())?;
                if let Ok(noun) = slot(noun, axis as u64) {
                    shell.process_out(false, noun);
                } else {
                    println!("Unable to cue - invalid axis");
                }
            }
            Command::Eval(slab) => {
                tx.send_async(SendSlab(slab)).await?;
                let slab = effects.recv_async().await?.0;
                let effect = unsafe { slab.root() };

                let handle_eval =
                    |shell: &mut ShellState, effect: Noun| -> core::result::Result<(), JetErr> {
                        let cell = effect.as_cell()?;

                        match cell.head().as_either_atom_cell() {
                            Either::Left(a) => {
                                match std::str::from_utf8(a.as_ne_bytes())
                                    .map(|v| v.trim_end_matches('\0'))
                                {
                                    Ok("poke") => {
                                        println!("Error running command");
                                    }
                                    _ => println!("Unrecognized result"),
                                }
                            }
                            Either::Right(cell) => {
                                let res = cell.tail();
                                match cell
                                    .head()
                                    .as_atom()
                                    .map(|a| a.to_le_bytes())
                                    .as_deref()
                                    .ok()
                                    .and_then(|v| std::str::from_utf8(v).ok())
                                    .map(|v| v.trim_end_matches('\0'))
                                {
                                    Some("jojo") => {
                                        let vase = slot(res, 2).unwrap();
                                        if shell.process_out(true, vase) {
                                            let pretty = slot(res, 3).unwrap().as_atom().unwrap();
                                            let pretty = pretty.as_ne_bytes();
                                            let pretty = std::str::from_utf8(pretty)
                                                .unwrap()
                                                .trim_end_matches('\0');
                                            println!("{pretty}");
                                        }
                                    }
                                    v => {
                                        println!("Unrecognized result {v:?}");
                                    }
                                }
                            }
                        }

                        Ok(())
                    };

                handle_eval(&mut shell, *effect).unwrap();
            }
        }
    }

    task.await?
}

#[tokio::main]
async fn main() -> Result<()> {
    nockvm::check_endian();
    let cli = JojoCli::parse();
    boot::init_default_tracing(&cli.nockapp_cli);

    match cli.mode {
        Mode::Jojo(j) => j.run(cli.nockapp_cli).await,
        Mode::Eval(e) => e.run(cli.nockapp_cli).await,
        Mode::Shell => shell(cli.nockapp_cli).await,
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
