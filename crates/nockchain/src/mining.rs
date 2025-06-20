use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use kernels::miner::KERNEL;
use nockapp::kernel::boot::TraceOpts;
use nockapp::kernel::checkpoint::JamPaths;
use nockapp::kernel::form::Kernel;
use nockapp::nockapp::driver::{IODriverFn, NockAppHandle, PokeResult};
use nockapp::nockapp::wire::Wire;
use nockapp::nockapp::NockAppError;
use nockapp::noun::slab::NounSlab;
use nockapp::noun::{AtomExt, NounExt};
use nockapp::Bytes;
use nockvm::noun::{Atom, FullDebugCell, D, NO, T, YES};
use nockvm_macros::tas;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::{debug, error, instrument, trace, warn};
use zkvm_jetpack::noun::noun_ext::NounExt as ZNounExt;

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

#[derive(Debug, Clone)]
pub struct MiningKeyConfig {
    pub share: u64,
    pub m: u64,
    pub keys: Vec<String>,
}

impl FromStr for MiningKeyConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expected format: "share,m:key1,key2,key3"
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid format. Expected 'share,m:key1,key2,key3'".to_string());
        }

        let share_m: Vec<&str> = parts[0].split(',').collect();
        if share_m.len() != 2 {
            return Err("Invalid share,m format".to_string());
        }

        let share = share_m[0].parse::<u64>().map_err(|e| e.to_string())?;
        let m = share_m[1].parse::<u64>().map_err(|e| e.to_string())?;
        let keys: Vec<String> = parts[1].split(',').map(String::from).collect();

        Ok(MiningKeyConfig { share, m, keys })
    }
}

pub fn create_mining_driver(
    mining_config: Option<Vec<MiningKeyConfig>>,
    mine: bool,
    init_complete_tx: Option<tokio::sync::oneshot::Sender<()>>,
    miners: usize,
    pin_miner_threads: Vec<usize>,
    trc: TraceOpts,
    fakenet: bool,
) -> IODriverFn {
    Box::new(move |mut handle| {
        Box::pin(async move {
            let Some(configs) = mining_config else {
                enable_mining(&handle, false).await?;

                if let Some(tx) = init_complete_tx {
                    tx.send(()).map_err(|_| {
                        warn!("Could not send driver initialization for mining driver.");
                        NockAppError::OtherError
                    })?;
                }

                return Ok(());
            };
            if configs.len() == 1
                && configs[0].share == 1
                && configs[0].m == 1
                && configs[0].keys.len() == 1
            {
                set_mining_key(&handle, configs[0].keys[0].clone()).await?;
            } else {
                set_mining_key_advanced(&handle, configs).await?;
            }
            enable_mining(&handle, mine).await?;

            if let Some(tx) = init_complete_tx {
                tx.send(()).map_err(|_| {
                    warn!("Could not send driver initialization for mining driver.");
                    NockAppError::OtherError
                })?;
            }

            if !mine {
                return Ok(());
            }
            let mut next_attempt: Option<NounSlab> = None;
            let mut current_attempt: tokio::task::JoinSet<(
                MiningPool,
                Option<Vec<(NounSlab, usize, Bytes)>>,
            )> = tokio::task::JoinSet::new();
            let mut current_result_poke: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards");
            let run_id = now.as_secs().to_string();
            let mut run_cnt = 0;

            let mut pool = Some(MiningPool::new(miners, trc, pin_miner_threads).await);

            loop {
                tokio::select! {
                    effect_res = handle.next_effect() => {
                        let Ok(effect) = effect_res else {
                          warn!("Error receiving effect in mining driver: {effect_res:?}");
                        continue;
                        };
                        let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
                            drop(effect);
                            continue;
                        };

                        if effect_cell.head().eq_bytes("mine") {
                            let candidate_slab = {
                                let mut slab = NounSlab::new();
                                slab.copy_into(effect_cell.tail());
                                slab
                            };
                            if !current_attempt.is_empty() {
                                next_attempt = Some(candidate_slab);
                            } else {
                                current_attempt.spawn(pool.take().unwrap().mining_attempt(candidate_slab));
                            }
                        }
                    },
                    poke_result = current_result_poke.join_next(), if !current_result_poke.is_empty() => {
                        if let Some(Err(e)) = poke_result {
                            warn!("Error during mining result poke: {e:?}");
                        }
                    }
                    mining_attempt_res = current_attempt.join_next(), if !current_attempt.is_empty() => {
                        match mining_attempt_res {
                            Some(Err(e)) => warn!("Error during mining attempt: {e:?}"),
                            Some(Ok((p, ret))) => {
                                if !current_result_poke.is_empty() {
                                    debug!("Previous result has not been processed, awaiting...");
                                    let _ = current_result_poke.join_next().await;
                                }
                                let (cur_handle, result_handle) = handle.dup();
                                handle = cur_handle;
                                if let Some(ret) = ret {
                                    current_result_poke.spawn(result_poke(ret, result_handle, run_id.clone(), fakenet, run_cnt));
                                }
                                run_cnt += 1;
                                pool = Some(p);
                            }
                            _ => (),
                        }
                        let Some(candidate_slab) = next_attempt else {
                            continue;
                        };
                        next_attempt = None;
                        current_attempt.spawn(pool.take().unwrap().mining_attempt(candidate_slab));
                    }
                }
            }
        })
    })
}

pub async fn result_poke(
    ret: Vec<(NounSlab, usize, Bytes)>,
    handle: NockAppHandle,
    run_id: String,
    fakenet: bool,
    run_cnt: usize,
) {
    for (effects_slab, miner_id, candidate_jam) in ret {
        for effect in effects_slab.to_vec() {
            let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
                drop(effect);
                continue;
            };
            unsafe {
                trace!(
                    "Miner {miner_id} effect {:?}",
                    effect.root().as_cell().as_ref().map(FullDebugCell)
                );
            }
            if effect_cell.head().eq_bytes("command") {
                if miner_id == 0 {
                    let dir = std::path::Path::new("miner_jams")
                        .join(if fakenet { "fakenet" } else { "mainnet" })
                        .join(&run_id)
                        .join(run_cnt.to_string());
                    tokio::fs::create_dir_all(&dir).await.unwrap();
                    let _ = tokio::fs::write(dir.join("event.jam"), candidate_jam.clone()).await;
                    let _ = tokio::fs::write(dir.join("effect.jam"), effect.jam()).await;
                }
                handle
                    .poke(MiningWire::Mined.to_wire(), effect)
                    .await
                    .expect("Could not poke nockchain with mined PoW");
            }
        }
    }
}

struct MiningPool {
    miner_loops: Vec<JoinHandle<()>>,
    tx: Vec<Sender<NounSlab>>,
    rx: Vec<Receiver<NounSlab>>,
}

impl Drop for MiningPool {
    fn drop(&mut self) {
        for l in &self.miner_loops {
            l.abort();
        }
    }
}

impl MiningPool {
    pub async fn new(miners: usize, trc: TraceOpts, pin_miner_threads: Vec<usize>) -> Self {
        let mut miner_loops = vec![];
        let mut tx = vec![];
        let mut rx = vec![];
        for i in 0..miners {
            let (out, r) = mpsc::channel(1);
            let (t, inp) = mpsc::channel(1);
            let trc = trc.clone();
            let pin_thread = if pin_miner_threads.is_empty() {
                None
            } else if pin_miner_threads.len() == 1 {
                Some(pin_miner_threads[0] + i)
            } else {
                Some(pin_miner_threads[i])
            };
            miner_loops.push(tokio::spawn(async move {
                let miner = Miner::new(trc, pin_thread).await;
                miner.mine_loop(inp, out).await
            }));
            tx.push(t);
            rx.push(r);
        }
        Self {
            miner_loops,
            tx,
            rx,
        }
    }

    pub async fn mining_attempt(
        mut self,
        candidate: NounSlab,
    ) -> (Self, Option<Vec<(NounSlab, usize, Bytes)>>) {
        let ret = self.mining_attempt_inner(candidate).await;
        (self, ret)
    }

    async fn mining_attempt_inner(
        &mut self,
        candidate: NounSlab,
    ) -> Option<Vec<(NounSlab, usize, Bytes)>> {
        debug!("New mining attempt");

        let mut jset = vec![];

        let r = unsafe { candidate.root() };
        let Ok([version, block_commitment, nonce, length]) = r.uncell() else {
            error!("Invalid mining request sent!");
            return None;
        };

        let Ok(nonce) = nonce.uncell::<5>() else {
            error!("Invalid nonce sent!");
            return None;
        };

        for (id, (tx, rx)) in self.tx.iter().zip(self.rx.iter_mut()).enumerate() {
            // Modify the nonce for the attempt
            let mut slab = candidate.clone();
            // Permute the nonce
            let pnid = id % 5;
            let n = nonce[pnid];
            let Ok(n) = n.as_atom().and_then(|n| n.as_u64()) else {
                error!("Cannot parse nonce part {pnid} as atom! ({n:?})");
                return None;
            };
            let n = n ^ (id as u64);
            let mut nonce = nonce;
            nonce[pnid] = Atom::new(&mut slab, n).as_noun();
            let nonce = T(&mut slab, &nonce);
            let candidate = T(&mut slab, &[version, block_commitment, nonce, length]);
            slab.copy_into(candidate);
            jset.push(async move {
                let jam = slab.jam();
                tx.send(slab).await.ok()?;
                rx.recv().await.zip(Some((id, jam)))
            });
        }

        let ret = join_all(jset).await;

        Some(
            ret.into_iter()
                .filter_map(|v| v.map(|(a, (b, c))| (a, b, c)))
                .collect(),
        )
    }
}

struct Miner {
    kernel: Kernel,
    snapshot_dir: TempDir,
}

impl Miner {
    pub async fn new(trc: TraceOpts, pin_thread: Option<usize>) -> Self {
        let snapshot_dir = tokio::task::spawn_blocking(|| {
            tempdir().expect("Failed to create temporary directory")
        })
        .await
        .expect("Failed to create temporary directory");
        let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
        let snapshot_path_buf = snapshot_dir.path().to_path_buf();
        let jam_paths = JamPaths::new(snapshot_dir.path());
        // Spawns a new std::thread for this mining attempt
        let kernel = Kernel::load_with_hot_state(
            snapshot_path_buf,
            jam_paths,
            KERNEL,
            &hot_state,
            trc.into(),
        )
        .await
        .expect("Could not load mining kernel");

        if let Some(pin_thread) = pin_thread {
            kernel
                .set_affinity(vec![pin_thread])
                .await
                .expect("Unable to set affinity for miner");
        }

        Self {
            kernel,
            snapshot_dir,
        }
    }

    pub async fn mine_loop(self, mut inp: Receiver<NounSlab>, out: Sender<NounSlab>) {
        let Self {
            kernel,
            snapshot_dir: _,
        } = self;

        while let Some(candidate) = inp.recv().await {
            let effects_slab = kernel
                .poke(MiningWire::Candidate.to_wire(), candidate)
                .await
                .expect("Could not poke mining kernel with candidate");
            if out.send(effects_slab).await.is_err() {
                break;
            }
        }
    }
}

#[instrument(skip(handle, pubkey))]
async fn set_mining_key(
    handle: &NockAppHandle,
    pubkey: String,
) -> Result<PokeResult, NockAppError> {
    let mut set_mining_key_slab = NounSlab::new();
    let set_mining_key = Atom::from_value(&mut set_mining_key_slab, "set-mining-key")
        .expect("Failed to create set-mining-key atom");
    let pubkey_cord =
        Atom::from_value(&mut set_mining_key_slab, pubkey).expect("Failed to create pubkey atom");
    let set_mining_key_poke = T(
        &mut set_mining_key_slab,
        &[
            D(tas!(b"command")),
            set_mining_key.as_noun(),
            pubkey_cord.as_noun(),
        ],
    );
    set_mining_key_slab.set_root(set_mining_key_poke);

    handle
        .poke(MiningWire::SetPubKey.to_wire(), set_mining_key_slab)
        .await
}

async fn set_mining_key_advanced(
    handle: &NockAppHandle,
    configs: Vec<MiningKeyConfig>,
) -> Result<PokeResult, NockAppError> {
    let mut set_mining_key_slab = NounSlab::new();
    let set_mining_key_adv = Atom::from_value(&mut set_mining_key_slab, "set-mining-key-advanced")
        .expect("Failed to create set-mining-key-advanced atom");

    // Create the list of configs
    let mut configs_list = D(0);
    for config in configs {
        // Create the list of keys
        let mut keys_noun = D(0);
        for key in config.keys {
            let key_atom =
                Atom::from_value(&mut set_mining_key_slab, key).expect("Failed to create key atom");
            keys_noun = T(&mut set_mining_key_slab, &[key_atom.as_noun(), keys_noun]);
        }

        // Create the config tuple [share m keys]
        let config_tuple = T(
            &mut set_mining_key_slab,
            &[D(config.share), D(config.m), keys_noun],
        );

        configs_list = T(&mut set_mining_key_slab, &[config_tuple, configs_list]);
    }

    let set_mining_key_poke = T(
        &mut set_mining_key_slab,
        &[
            D(tas!(b"command")),
            set_mining_key_adv.as_noun(),
            configs_list,
        ],
    );
    set_mining_key_slab.set_root(set_mining_key_poke);

    handle
        .poke(MiningWire::SetPubKey.to_wire(), set_mining_key_slab)
        .await
}

//TODO add %set-mining-key-multisig poke
#[instrument(skip(handle))]
async fn enable_mining(handle: &NockAppHandle, enable: bool) -> Result<PokeResult, NockAppError> {
    let mut enable_mining_slab = NounSlab::new();
    let enable_mining = Atom::from_value(&mut enable_mining_slab, "enable-mining")
        .expect("Failed to create enable-mining atom");
    let enable_mining_poke = T(
        &mut enable_mining_slab,
        &[
            D(tas!(b"command")),
            enable_mining.as_noun(),
            if enable { YES } else { NO },
        ],
    );
    enable_mining_slab.set_root(enable_mining_poke);
    handle
        .poke(MiningWire::Enable.to_wire(), enable_mining_slab)
        .await
}
