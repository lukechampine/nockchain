use std::str::FromStr;
use std::sync::Mutex as SyncMutex;

use clap::{value_parser, Args};
use gdt_cpus::CoreType;
use kernels::miner::KERNEL;
use nockapp::kernel::form::SerfThread;
use nockapp::nockapp::driver::{IODriverFn, NockAppHandle, PokeResult};
use nockapp::nockapp::wire::Wire;
use nockapp::nockapp::NockAppError;
use nockapp::noun::slab::NounSlab;
use nockapp::noun::{AtomExt, NounExt};
use nockapp::save::SaveableCheckpoint;
use nockapp::utils::NOCK_STACK_SIZE_TINY;
use nockapp::CrownError;
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::interpreter::NockCancelToken;
use nockvm::jets::hot::HotEntry;
use nockvm::noun::{Atom, D, NO, T, YES};
use nockvm_macros::tas;
use rand::Rng;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument, warn};
use zkvm_jetpack::form::PRIME;
use zkvm_jetpack::noun::noun_ext::NounExt as OtherNounExt;

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

#[derive(Debug, Clone)]
pub enum PinThreads {
    // Pin threads in sequnece starting from starting core ID
    Sequence { start_core_id: usize },
    // Pin threads to exact logical core IDs
    Exact { core_ids: Vec<usize> },
    // Pin threads to performance cores
    Performance,
}

impl FromStr for PinThreads {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once("=").unwrap_or((s, "")) {
            ("sequence", v) => Ok(Self::Sequence {
                start_core_id: v
                    .parse()
                    .map_err(|_| format!("Invalid starting core id: {v}"))?,
            }),
            ("exact", v) => {
                let mut core_ids = vec![];
                for (i, c) in v.split(",").enumerate() {
                    core_ids.push(
                        c.parse()
                            .map_err(|_| format!("Invalid core ID at position {i}: {c}"))?,
                    );
                }
                Ok(Self::Exact { core_ids })
            }
            ("performance", "") => Ok(Self::Performance),
            _ => Err(
                "Invalid format. Expected sequence=starting_core, exact=core1,core2,core3, or performance"
                    .to_string(),
            ),
        }
    }
}

impl PinThreads {
    pub fn logical_core_ids(&self, num_threads: usize) -> Vec<usize> {
        match self {
            Self::Sequence { start_core_id } => (*start_core_id..).take(num_threads).collect(),
            Self::Exact { core_ids } => {
                assert_eq!(
                    core_ids.len(),
                    num_threads,
                    "Number of exact core IDs does not match the number of miner threads"
                );
                core_ids.clone()
            }
            Self::Performance => {
                let cpu_info = gdt_cpus::cpu_info().expect("Unable to query CPU info");
                let is_hybrid = cpu_info.is_hybrid();

                let mut cores = cpu_info
                    .sockets
                    .iter()
                    .flat_map(|v| v.cores.iter())
                    .filter(|v| !is_hybrid || v.core_type == CoreType::Performance)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut core_ids = vec![];

                // Fairly complicated loop to balance over different physical cores in SMT
                // scenarios, and only pinning to the same hyperthread if we run out of physical
                // cores.
                'outer: loop {
                    let start_len = core_ids.len();
                    for c in &mut cores {
                        if let Some(id) = c.logical_processor_ids.pop() {
                            core_ids.push(id);
                        }
                        if core_ids.len() == num_threads {
                            break 'outer;
                        }
                    }
                    assert_ne!(start_len, core_ids.len(), "Number of performance cores on the machine insufficient for given miner threads");
                }

                core_ids
            }
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct MiningConfig {
    #[arg(long, help = "Mine in-kernel", default_value = "false")]
    pub mine: bool,
    #[arg(long, help = "Number of threads to mine with defaults to one less than the number of cpus available.", default_value = None)]
    pub num_threads: Option<u64>,
    #[arg(
        long,
        help = "Pubkey to mine to (mutually exclusive with --mining-key-adv)"
    )]
    pub mining_pubkey: Option<String>,
    #[arg(
        long,
        help = "Advanced mining key configuration (mutually exclusive with --mining-pubkey). Format: share,m:key1,key2,key3",
        value_parser = value_parser!(MiningKeyConfig),
        num_args = 1..,
    )]
    pub mining_key_adv: Option<Vec<MiningKeyConfig>>,
    #[arg(
        long,
        help = "Pin miner threads to given CPU cores. Format: sequence=starting_core, exact=core1,core2,core3, or performance"
    )]
    pub pin_threads: Option<PinThreads>,
}

impl MiningConfig {
    pub fn mining_key_config(&self) -> Option<Vec<MiningKeyConfig>> {
        if let Some(pubkey) = &self.mining_pubkey {
            Some(vec![MiningKeyConfig {
                share: 1,
                m: 1,
                keys: vec![pubkey.clone()],
            }])
        } else if let Some(mining_key_adv) = &self.mining_key_adv {
            Some(mining_key_adv.clone())
        } else {
            None
        }
    }

    pub fn num_threads(&self) -> u64 {
        self.num_threads.unwrap_or(1)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mine && !(self.mining_pubkey.is_some() || self.mining_key_adv.is_some()) {
            return Err(
                "Cannot specify mine without either mining_pubkey or mining_key_adv".to_string(),
            );
        }

        if self.mining_pubkey.is_some() && self.mining_key_adv.is_some() {
            return Err(
                "Cannot specify both mining_pubkey and mining_key_adv at the same time".to_string(),
            );
        }

        Ok(())
    }
}

struct MiningData {
    pub block_header: NounSlab,
    pub version: NounSlab,
    pub target: NounSlab,
    pub pow_len: u64,
}

pub fn create_mining_driver(
    cfg: MiningConfig,
    init_complete_tx: Option<tokio::sync::oneshot::Sender<()>>,
) -> IODriverFn {
    Box::new(move |handle| {
        Box::pin(async move {
            let Some(configs) = cfg.mining_key_config() else {
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
            enable_mining(&handle, cfg.mine).await?;

            if let Some(tx) = init_complete_tx {
                tx.send(()).map_err(|_| {
                    warn!("Could not send driver initialization for mining driver.");
                    NockAppError::OtherError
                })?;
            }

            if !cfg.mine {
                return Ok(());
            }

            let num_threads = cfg.num_threads();
            info!("Starting mining driver with {} threads", num_threads);

            let pin_threads = cfg
                .pin_threads
                .map(|v| v.logical_core_ids(num_threads as _));

            let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
            let test_jets_str = std::env::var("NOCK_TEST_JETS").unwrap_or_default();
            let test_jets = nockapp::kernel::boot::parse_test_jets(test_jets_str.as_str());

            let mining_data: Mutex<Option<MiningData>> = Mutex::new(None);

            let (mining_attempt_results, mut mining_attempts) = mpsc::channel(num_threads as usize);

            let mut miners = tokio::task::JoinSet::new();
            for i in 0..(num_threads as usize) {
                let core_id = pin_threads.as_ref().map(|v| v[i]);
                let miner_fut = MinerHandle::new(
                    hot_state.clone(),
                    test_jets.clone(),
                    i,
                    core_id,
                    mining_attempt_results.clone(),
                );
                miners.spawn(miner_fut);
            }
            let miners = miners.join_all().await;

            loop {
                tokio::select! {
                        mining_result = mining_attempts.recv() => {
                            let (id, slab_res) = mining_result.expect("Mining attempt result failed");
                            let miner = &miners[id];
                            let slab = slab_res.expect("Mining attempt result failed");
                            let result = unsafe { slab.root() };
                            // If the mining attempt was cancelled, the goof goes into poke_swap which returns
                            // %poke followed by the cancelled poke. So we check for hed = %poke
                            // to identify a cancelled attempt.
                            let hed = result.as_cell().expect("Expected result to be a cell").head();
                            if hed.is_atom() && hed.eq_bytes("poke") {
                                //  mining attempt was cancelled. restart with current block header.
                                debug!("mining attempt cancelled. restarting on new block header. thread={id}");
                                start_mining_attempt(miner, mining_data.lock().await, None).await;
                            } else {
                                //  there should only be one effect
                                let effect = result.as_cell().expect("Expected result to be a cell").head();
                                let [head, res, tail] = effect.uncell().expect("Expected three elements in mining result");
                                if head.eq_bytes("mine-result") {
                                    if unsafe { res.raw_equals(&D(0)) } {
                                        // success
                                        // poke main kernel with mined block. Do not start a new attempt,
                                        // because it will be invalid anyways.
                                        info!("Found block! thread={id}");
                                        let [_, poke] = tail.uncell().expect("Expected two elements in tail");
                                        let mut poke_slab = NounSlab::new();
                                        poke_slab.copy_into(poke);
                                        handle.poke(MiningWire::Mined.to_wire(), poke_slab).await.expect("Could not poke nockchain with mined PoW");
                                    } else {
                                        // failure
                                        //  launch new attempt, using hash as new nonce
                                        //  nonce is tail
                                        debug!("didn't find block, starting new attempt. thread={id}");
                                        let mut nonce_slab = NounSlab::new();
                                        nonce_slab.copy_into(tail);
                                        start_mining_attempt(miner, mining_data.lock().await, Some(nonce_slab)).await;
                                    }
                                }
                            }
                        }

                    effect_res = handle.next_effect() => {
                        let effect = match effect_res {
                            Ok(effect) => effect,
                            Err(NockAppError::BroadcastRecvClosedError) => {
                                info!("Nockapp closing");
                                break;
                            }
                            Err(e) => {
                                warn!("Error receiving effect in mining driver: {e:?}");
                                continue;
                            }
                        };
                        let Ok(effect_cell) = (unsafe { effect.root().as_cell() }) else {
                            drop(effect);
                            continue;
                        };

                        if effect_cell.head().eq_bytes("mine") {
                            let (version_slab, header_slab, target_slab, pow_len) = {
                                let [version, commit, target, pow_len_noun] = effect_cell.tail().uncell().expect(
                                    "Expected three elements in %mine effect",
                                );
                                let mut version_slab = NounSlab::new();
                                version_slab.copy_into(version);
                                let mut header_slab = NounSlab::new();
                                header_slab.copy_into(commit);
                                let mut target_slab = NounSlab::new();
                                target_slab.copy_into(target);
                                let pow_len =
                                    pow_len_noun
                                        .as_atom()
                                        .expect("Expected pow-len to be an atom")
                                        .as_u64()
                                        .expect("Expected pow-len to be a u64");
                                (version_slab, header_slab, target_slab, pow_len)
                            };
                            debug!("received new candidate block header: {:?}",
                                tip5_hash_to_base58(*unsafe { header_slab.root() })
                                .expect("Failed to convert header to Base58")
                            );
                            *(mining_data.lock().await) = Some(MiningData {
                                block_header: header_slab,
                                version: version_slab,
                                target: target_slab,
                                pow_len: pow_len
                            });

                            // Cancel any existing mining attempts, and create new attempts
                            for m in &miners {
                                start_mining_attempt(m, mining_data.lock().await, None).await;
                            }
                        }
                    }
                }
            }

            debug!("Stopping miner threads");
            core::mem::drop(mining_attempts);
            for m in miners {
                m.finish().await;
            }

            Ok(())
        })
    })
}

fn create_poke(mining_data: &MiningData, nonce: &NounSlab) -> NounSlab {
    let mut slab = NounSlab::new();
    let header = slab.copy_into(unsafe { *(mining_data.block_header.root()) });
    let version = slab.copy_into(unsafe { *(mining_data.version.root()) });
    let target = slab.copy_into(unsafe { *(mining_data.target.root()) });
    let nonce = slab.copy_into(unsafe { *(nonce.root()) });
    let poke_noun = T(
        &mut slab,
        &[version, header, nonce, target, D(mining_data.pow_len)],
    );
    slab.set_root(poke_noun);
    slab
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
        &[D(tas!(b"command")), set_mining_key.as_noun(), pubkey_cord.as_noun()],
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
        &[D(tas!(b"command")), set_mining_key_adv.as_noun(), configs_list],
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
        &[D(tas!(b"command")), enable_mining.as_noun(), if enable { YES } else { NO }],
    );
    enable_mining_slab.set_root(enable_mining_poke);
    handle
        .poke(MiningWire::Enable.to_wire(), enable_mining_slab)
        .await
}

async fn start_mining_attempt(
    miner: &MinerHandle,
    mining_data: tokio::sync::MutexGuard<'_, Option<MiningData>>,
    nonce: Option<NounSlab>,
) {
    let nonce = nonce.unwrap_or_else(|| {
        let mut rng = rand::thread_rng();
        let mut nonce_slab = NounSlab::new();
        let mut nonce_cell = Atom::from_value(&mut nonce_slab, rng.gen::<u64>() % PRIME)
            .expect("Failed to create nonce atom")
            .as_noun();
        for _ in 1..5 {
            let nonce_atom = Atom::from_value(&mut nonce_slab, rng.gen::<u64>() % PRIME)
                .expect("Failed to create nonce atom")
                .as_noun();
            nonce_cell = T(&mut nonce_slab, &[nonce_atom, nonce_cell]);
        }
        nonce_slab.set_root(nonce_cell);
        nonce_slab
    });
    let mining_data_ref = mining_data
        .as_ref()
        .expect("Mining data should already be initialized");
    debug!(
        "starting mining attempt on thread {:?} on header {:?}with nonce: {:?}",
        miner.id,
        tip5_hash_to_base58(*unsafe { mining_data_ref.block_header.root() })
            .expect("Failed to convert block header to Base58"),
        tip5_hash_to_base58(*unsafe { nonce.root() }).expect("Failed to convert nonce to Base58"),
    );
    let poke_slab = create_poke(mining_data_ref, &nonce);
    miner.send_poke(poke_slab);
}

struct Miner {
    serf: SerfThread<SaveableCheckpoint>,
    id: usize,
    results: mpsc::Sender<(usize, Result<NounSlab, CrownError>)>,
    reqs: watch::Receiver<SyncMutex<Option<NounSlab>>>,
}

impl Miner {
    pub async fn run(mut self) {
        while self.reqs.changed().await.is_ok() {
            let Some(poke_slab) = ({
                let mtx = self.reqs.borrow_and_update();
                let mut guard = mtx.lock().expect("Poisoned lock");
                guard.take()
            }) else {
                continue;
            };

            let result = self
                .serf
                .poke(MiningWire::Candidate.to_wire(), poke_slab)
                .await;

            if self.results.send((self.id, result)).await.is_err() {
                break;
            }
        }
    }
}

struct MinerHandle {
    reqs: watch::Sender<SyncMutex<Option<NounSlab>>>,
    cancellation: NockCancelToken,
    miner_loop: JoinHandle<()>,
    id: usize,
}

impl MinerHandle {
    pub async fn new(
        hot_state: Vec<HotEntry>,
        test_jets: Vec<NounSlab>,
        id: usize,
        thread_pin: Option<usize>,
        results: mpsc::Sender<(usize, Result<NounSlab, CrownError>)>,
    ) -> Self {
        let kernel = Vec::from(KERNEL);
        let serf = SerfThread::<SaveableCheckpoint>::new(
            kernel,
            None,
            hot_state,
            NOCK_STACK_SIZE_TINY,
            test_jets,
            Default::default(),
        )
        .await
        .expect("Could not load mining kernel");

        let cancellation = serf.cancel_token.clone();

        if let Some(core_id) = thread_pin {
            serf.call_fn(move || gdt_cpus::pin_thread_to_core(core_id))
                .await
                .expect("Could not invoke core pinning")
                .expect("Could not pin the miner thread");
        }

        let (tx, rx) = watch::channel(SyncMutex::new(None));

        let miner = Miner {
            serf,
            id,
            results,
            reqs: rx,
        };

        let miner_loop = tokio::spawn(miner.run());

        Self {
            reqs: tx,
            cancellation,
            miner_loop,
            id,
        }
    }

    pub fn send_poke(&self, poke_slab: NounSlab) {
        self.reqs.send_modify(|v| {
            let mut guard = v.lock().expect("Poisoned lock");
            *guard = Some(poke_slab);
            // Cancel while holding the guard to prevent the miner from racing to a stale request.
            self.cancel_current_poke();
        });
    }

    pub fn cancel_current_poke(&self) {
        self.cancellation.cancel();
    }

    pub async fn finish(self) {
        self.cancel_current_poke();
        let _ = self.miner_loop.await;
    }
}
