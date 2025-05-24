use std::error::Error;
use std::path::Path;

use clap::Parser;
use either::Either;
use nockapp::kernel::boot;
use nockapp::kernel::checkpoint::JamPaths;
use nockapp::utils::{create_context, NOCK_STACK_SIZE_HUGE};
use nockapp::Noun;
use nockvm::interpreter::interpret;
use nockvm::jets::cold::Cold;
use nockvm::jets::hot::URBIT_HOT_STATE;
use nockvm::mem::NockStack;
use nockvm::noun::{DirectAtom, FullDebugCell, IndirectAtom, D, T};
use nockvm::serialization::cue;
use nockvm::trace::path_to_cord;
use nockvm::unifying_equality::{self, unifying_equality};
use tracing::{debug, info};
use zkvm_jetpack::hot::produce_prover_hot_state;

/// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "nockchain")]
pub struct JettestCli {
    #[command(flatten)]
    pub nockapp_cli: nockapp::kernel::boot::Cli,
    #[arg(short, long, default_value = ".")]
    pub jamdir: String,
    #[arg(short, long)]
    pub interpret: bool,
    #[arg(short, long)]
    pub jet_run: bool,
    #[arg(long)]
    pub snapshot_dir: Option<String>,
}

fn load_jam(stack: &mut NockStack, path: impl AsRef<Path>) -> Result<Noun, Box<dyn Error>> {
    let jam = std::fs::read(path)?;
    let jam = unsafe { IndirectAtom::new_raw_bytes(stack, jam.len(), jam.as_ptr()) };
    let cue = cue(stack, jam.as_atom()).unwrap();
    Ok(cue)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    nockvm::check_endian();
    let cli = JettestCli::parse();
    boot::init_default_tracing(&cli.nockapp_cli);

    let hot_state = produce_prover_hot_state();
    let hot_state = [URBIT_HOT_STATE, &hot_state].concat();

    let mut stack = NockStack::new(NOCK_STACK_SIZE_HUGE, 0);

    let cold = if let Some(snapshot_dir) = cli.snapshot_dir {
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
    } else {
        debug!("Snapshot dir unspecified, making empty cold state!");
        Cold::new(&mut stack)
    };

    let mut context = create_context(stack, &hot_state, cold, cli.nockapp_cli.trace_opts.into());

    let p = Path::new(&cli.jamdir);
    let subject = load_jam(&mut context.stack, p.join("subject.jam"))?;
    let formula = load_jam(&mut context.stack, p.join("formula.jam"))?;
    let jetpath = load_jam(&mut context.stack, p.join("jetpath.jam"))?;

    let jetcord = path_to_cord(&mut context.stack, jetpath);
    let jetcord = std::str::from_utf8(jetcord.as_ne_bytes()).unwrap_or("");
    debug!("Formula in question: {jetcord}");
    let jetpath = jetpath.as_cell()?;

    if cli.jet_run {
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

            if unsafe { unifying_equality(&mut context.stack, &mut jetpath.as_noun(), &mut a_path) } {
                eprintln!("Found Jet!");
                match jet(&mut context, subject) {
                    Ok(res) => {
                        eprintln!("Jet Ran OK");
                    }
                    Err(e) => {
                        eprintln!("ERROR JET: {e:?}");
                    }
                }
            }
        }

        // let res =
    }

    if cli.interpret {
        match interpret(&mut context, subject, formula) {
            Ok(res) => {
                eprintln!("Ran OK");
            }
            Err(e) => {
                eprintln!("ERROR INTERPRETING: {e:?}");
            }
        }
    }

    Ok(())
}
