use crate::parse::{parse_tree, Node};
use crate::{run_kernel, SendSlab};
use anyhow::{anyhow, Result};
use either::Either;
use flume::Sender;
use nockapp::kernel::boot::Cli;
use nockapp::noun::slab::NounSlab;
use nockapp::Bytes;
use nockvm::jets::util::slot;
use nockvm::jets::JetErr;
use nockvm::noun::*;
use nockvm_macros::tas;
use reedline::{DefaultPrompt, DefaultPromptSegment, FileBackedHistory, Reedline, Signal};
use std::collections::HashMap;

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
    in_subject: Option<Node>,
    out_sample: Option<String>,
    jam_out: Option<String>,
    cue_inp: Option<(String, usize)>,
    list_vars: bool,
}

impl ShellCommand {
    fn parse(mut line: &str) -> Result<Self> {
        let mut in_sample = None;
        let mut in_subject = None;
        let mut out_sample = None;
        let mut jam_out = None;
        let mut cue_inp = None;
        let mut list_vars = false;

        let hoon = if line.starts_with('/') {
            line = line.split_once('/').unwrap().1;

            loop {
                let (mut cmd, mut rest) = line.split_once(' ').unwrap_or((line, ""));

                let mut parse_vars = || {
                    let brack = rest.rfind(']');
                    let (cmd, mut rest) = brack
                        .map(|v| rest.split_at(v + 1))
                        .or_else(|| rest.split_once(' '))
                        .unwrap_or((rest, ""));

                    rest = rest.trim_ascii_start();

                    if cmd.is_empty() {
                        return Err(anyhow!("Variable is not specified"));
                    }

                    let Some(tree) = parse_tree(cmd) else {
                        return Err(anyhow!("Malformed var: {cmd}"));
                    };

                    Ok((Some(tree), cmd, rest))
                };

                match cmd {
                    "?" | "help" => {
                        return Err(anyhow!(
                            r"Usage:

/?, /help - display this message.
/. sam func - evaluate `func` with the given sample `sam`. Given sample can be constructed from variable assignments (using /.), and can be a cell, e.g. [a b].
/: sub hoon - evaluate `hoon` with subject `sub`. Subject can be constructed from variable assignments.
/= var hoon - evaluage `hoon` and assign output to `var`.
/+ v s hoon - evaluate `hoon` on subject `s`, and assign output to `v`. Subject can be constructed from variable assignments.
/p sam      - print given sample `sam`. Note: if single variable is provided, it is pretty-printed, but mutliple variables are printed as raw nouns.
/v          - list defined variables.
/c var path - cue a jamfile at `path` and assign it to `var`.
/s var path - cue a subject jamfile at `path`, take the sample at axis 6, and assign it to `var`.
/j sam path - jam the given sample `sam` and write it to `path`.
hoon - evaluate `hoon` and print the result out to screen.
                            "
                        ))
                    }
                    "." => (in_sample, _, line) = parse_vars()?,
                    ":" => (in_subject, _, line) = parse_vars()?,
                    "=" => {
                        (cmd, line) = rest.split_once(' ').unwrap_or((rest, ""));
                        if cmd.is_empty() {
                            return Err(anyhow!("Sample is not specified"));
                        }
                        out_sample = Some(cmd.to_string());
                    }
                    "+" => {
                        (in_sample, _, _) = parse_vars()?;
                        (in_subject, _, line) = parse_vars()?;
                    }
                    "p" => {
                        (in_sample, _, _) = parse_vars()?;
                        break None;
                    }
                    "v" => {
                        list_vars = true;
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
                        (in_sample, _, line) = parse_vars()?;
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
            if !hoon.starts_with(char::is_alphabetic) || !hoon.contains(' ') {
                hoon.to_string()
            } else {
                format!("({hoon})")
            }
        });

        Ok(Self {
            hoon,
            in_sample,
            in_subject,
            out_sample,
            jam_out,
            cue_inp,
            list_vars,
        })
    }
}

#[derive(Default)]
pub struct Shell {
    samples: HashMap<String, (bool, NounSlab)>,
    save_state: Option<String>,
}

enum Command {
    Eval(NounSlab),
    Jam(Bytes, String),
    Cue(String, usize),
    ListVars(Vec<String>),
}

impl Shell {
    fn next_command(&mut self, line: &str) -> Result<Command> {
        let ShellCommand {
            hoon,
            in_sample,
            in_subject,
            out_sample,
            jam_out,
            cue_inp,
            list_vars,
        } = ShellCommand::parse(line)?;

        self.save_state = out_sample;

        let mut slab = NounSlab::new();

        let mut vases = [None, None];

        for (vas, sam) in vases.iter_mut().zip([in_sample, in_subject]) {
            if let Some(tree) = sam {
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
                    *vas = Some(noun);
                } else {
                    let noun = tree.fold(
                        &mut |sample: String| {
                            pull_noun(sample)
                                .map(|(vased, v)| if *vased { slot(v, 3).unwrap() } else { v })
                        },
                        &mut |nouns: Vec<Noun>| Ok(T(&mut slab, &nouns[..])),
                    )?;
                    *vas = Some(T(&mut slab, &[D(tas!(b"noun")), noun]));
                }
            }
        }

        let [vased_sample, vased_subject] = vases;

        let vased_subject = vased_subject
            .map(|v| T(&mut slab, &[D(0), v]))
            .unwrap_or(D(0));

        if let Some(jam) = jam_out {
            let sam = vased_sample
                .and_then(|v| slot(v, 3).ok())
                .ok_or_else(|| anyhow!("Cannot jam without sample (impossible)"))?;
            let slab = NounSlab::from(sam);
            let bytes = slab.jam();
            return Ok(Command::Jam(bytes, jam));
        } else if let Some((cue, axis)) = cue_inp {
            return Ok(Command::Cue(cue, axis));
        } else if list_vars {
            return Ok(Command::ListVars(self.samples.keys().cloned().collect()));
        }

        let hoon = hoon.as_deref().map(|v| unsafe {
            IndirectAtom::new_raw_bytes(&mut slab, v.len(), v.as_ptr()).as_noun()
        });

        let poke = match (hoon, vased_sample) {
            (Some(hoon), Some(sam)) => {
                let sam = slot(sam, 3).unwrap();
                slab.copy_into(sam);
                let sam = unsafe { *slab.root() };
                T(&mut slab, &[D(tas!(b"sam")), hoon, vased_subject, sam])
            }
            (Some(hoon), None) => T(&mut slab, &[D(tas!(b"raw")), hoon, vased_subject]),
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

    pub async fn run(mut self, cli: Cli) -> Result<()> {
        let (tx, rx) = flume::bounded(0);
        let effects = run_kernel(rx.into_stream(), cli).await;

        // Intentionally rendezvous!
        let (lines_out, lines) = flume::bounded(0);

        let task = tokio::task::spawn_blocking(move || input_func(lines_out));

        while let Ok(line) = lines.recv_async().await {
            let Some(line) = line else { continue };

            if line.is_empty() {
                continue;
            }

            let command = match self.next_command(&line) {
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
                        self.process_out(false, noun);
                    } else {
                        println!("Unable to cue - invalid axis");
                    }
                }
                Command::ListVars(vars) => {
                    println!("Defined variables:");
                    for v in vars {
                        println!("{v}");
                    }
                }
                Command::Eval(slab) => {
                    tx.send_async(SendSlab(slab)).await?;
                    let slab = effects.recv_async().await?.0;
                    let effect = unsafe { slab.root() };

                    let handle_eval = |shell: &mut Shell,
                                       effect: Noun|
                     -> core::result::Result<(), JetErr> {
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

                    handle_eval(&mut self, *effect).unwrap();
                }
            }
        }

        task.await?
    }
}
