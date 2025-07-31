use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use clap::Args;
use gdt_cpus::CoreType;
use kernels::miner::KERNEL;
use metrics::{counter, gauge, histogram, Gauge};
use nockapp::kernel::form::SerfThread;
use nockapp::nockapp::wire::Wire;
use nockapp::noun::slab::{NockJammer, NounSlab};
use nockapp::noun::{AtomExt, NounExt};
use nockapp::save::SaveableCheckpoint;
use nockapp::utils::NOCK_STACK_SIZE_TINY;
use nockapp::CrownError;
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::interpreter::NockCancelToken;
use nockvm::jets::hot::HotEntry;
use nockvm::noun::{Atom, D, T};
use rand::Rng;
use rustls::crypto::ring::default_provider;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::sleep;
use tracing::*;
use zkvm_jetpack::form::PRIME;
use zkvm_jetpack::noun::noun_ext::NounExt as OtherNounExt;

use crate::proto::{self, MiningAckOut, MiningDataOut, MiningResultIn};
use crate::shared::{tls_connect_wrap, MiningData, MiningResult, MiningWire, TlsClientConfig};

struct ServerExtras {
    mining_res: mpsc::Sender<MiningResultIn>,
    live: Arc<AtomicBool>,
}

pub async fn run_client(cfg: ClientConfig) {
    #[cfg(not(feature = "force-tls"))]
    let tls = if cfg.miner_connect_tls {
        let _ = default_provider().install_default();
        Some(Default::default())
    } else {
        None
    };

    #[cfg(feature = "force-tls")]
    let tls = {
        let _ = default_provider().install_default();
        Some(Default::default())
    };

    let num_threads = cfg.num_threads();
    info!("Starting mining driver with {} threads", num_threads);

    let pin_threads = cfg
        .pin_threads
        .map(|v| v.logical_core_ids(num_threads as _));

    let client_name = cfg.client_name.unwrap_or_default();

    let mut client_tasks = JoinSet::new();

    let (mining_tx, mut mining_rx) = mpsc::channel(cfg.miner_connect.len());
    let (ack_tx, mut ack_rx) = mpsc::channel(cfg.miner_connect.len());
    let mut server_extras = vec![];

    let (mining_attempt_results, mut mining_attempts) = mpsc::channel(num_threads as usize);

    let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
    let nbx_jets = nbx_jetpack::nbx_jets().collect::<Vec<_>>();
    let hot_state = [nbx_jets, hot_state].concat();
    let test_jets_str = std::env::var("NOCK_TEST_JETS").unwrap_or_default();
    let test_jets = nockapp::kernel::boot::parse_test_jets(test_jets_str.as_str());

    let mut miners = tokio::task::JoinSet::new();
    let mut miner_metadata = vec![];
    for i in 0..(num_threads as usize) {
        let core_id = pin_threads.as_ref().map(|v| v[i]);

        let mut mdata = BTreeMap::new();

        #[cfg(feature = "gpu")]
        let gpu = cfg.gpu_miners.iter().find(|v| v.miner == i).cloned();

        #[cfg(feature = "gpu")]
        if let Some(g) = &gpu {
            mdata.insert("gpu-index".to_string(), g.gpu_index.to_string());
            if let Some(filter) = &g.name_filter {
                mdata.insert("gpu-name".to_string(), filter.clone());
            }
        }

        let miner_fut = MinerHandle::new(
            hot_state.clone(),
            test_jets.clone(),
            i,
            core_id,
            mining_attempt_results.clone(),
            #[cfg(feature = "gpu")]
            gpu,
        );
        miners.spawn(miner_fut);
        miner_metadata.push(mdata);
    }
    let mut miners = miners.join_all().await;
    miners.sort_by_key(|v| v.id);

    for (i, a) in cfg.miner_connect.into_iter().enumerate() {
        let (tx, rx) = mpsc::channel(16);
        let live = Arc::new(AtomicBool::new(false));
        client_tasks.spawn(client_loop(
            a,
            tls,
            i,
            client_name.clone(),
            live.clone(),
            rx,
            mining_tx.clone(),
            ack_tx.clone(),
            miner_metadata.clone(),
        ));
        server_extras.push(ServerExtras {
            mining_res: tx,
            live,
        });
    }

    let mut requests = BTreeMap::new();

    loop {
        tokio::select! {
            v = mining_rx.recv() => {
                let MiningDataOut { server_id, data_id, session_id, data } = v.expect("Client loop died");
                requests.insert(server_id, (data, data_id, Instant::now(), session_id));
                for m in &miners {
                    start_mining_attempt(m, &server_extras, &mut requests).await;
                }
            }
            v = ack_rx.recv() => {
                let MiningAckOut { server_id, miner_id: _, data_id } = v.expect("Client loop died");
                if let Some(r) = requests.get_mut(&server_id) {
                    if r.1 == data_id {
                        r.2 = Instant::now();
                    }
                }
            }
            r = mining_attempts.recv() => {
                let MinerAttemptRes { id, duration, server_id, data_id, slab_res, slab_inp, session_id } = r.expect("Mining attempt result failed");
                let miner = &miners[id];
                let slab = slab_res.expect("Mining attempt result failed");
                let result = unsafe { slab.root() };
                // If the mining attempt was cancelled, the goof goes into poke_swap which returns
                // %poke followed by the cancelled poke. So we check for hed = %poke
                // to identify a cancelled attempt.
                let hed = result.as_cell().expect("Expected result to be a cell").head();
                if hed.is_cell() {
                    //  there should only be one effect
                    let effect = result.as_cell().expect("Expected result to be a cell").head();
                    let [head, res, _] = effect.uncell().expect("Expected three elements in mining result");
                    if head.eq_bytes("mine-result") {
                        let (is_block, poke, effect) = if unsafe { res.raw_equals(&D(0)) } {
                            (true, Some(slab_inp), Some(slab))
                        } else {
                            (false, None, None)
                        };

                        let extra = &server_extras[server_id];

                        if let Err(e) = extra.mining_res.send(MiningResultIn {
                            data_id,
                            session_id,
                            data: MiningResult {
                                miner_id: id,
                                attempt_seconds: duration.as_secs_f32(),
                                is_block,
                                poke,
                                effect,
                            }
                        }).await {
                            error!("Unable to send mining result to {server_id}: {e:?}");
                        }

                        // TODO: remove all mining requests and wait for new block height to come
                        // in. Also, make sure we receive the target block's height so that we can
                        // filter things out easier.
                    }
                }

                start_mining_attempt(miner, &server_extras, &mut requests).await;
            }
        }
    }
}

struct MinerAttemptRes {
    id: usize,
    duration: Duration,
    server_id: usize,
    data_id: usize,
    slab_res: Result<NounSlab, CrownError>,
    slab_inp: NounSlab,
    session_id: u32,
}

async fn client_loop(
    addr: SocketAddr,
    tls: Option<TlsClientConfig>,
    server_id: usize,
    client_name: String,
    live: Arc<AtomicBool>,
    mut results: mpsc::Receiver<MiningResultIn>,
    data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
    miner_metadata: Vec<BTreeMap<String, String>>,
) {
    let mut err_cnt = 0;
    loop {
        let stream = match tls_connect_wrap(TcpStream::connect(addr), tls).await {
            Ok(stream) => stream,
            Err(e) => {
                let sleep_secs = 1 << err_cnt;
                error!("Unable to connect to {addr}: {e:?}. Sleeping for {sleep_secs} seconds");
                sleep(Duration::from_secs(sleep_secs)).await;
                err_cnt = core::cmp::min(err_cnt + 1, 5);
                continue;
            }
        };

        let mut handshaked = false;

        live.fetch_or(true, Ordering::SeqCst);
        let res = proto::client(
            stream,
            server_id,
            client_name.clone(),
            &mut results,
            data.clone(),
            ack.clone(),
            miner_metadata.clone(),
            &mut handshaked,
        )
        .await;
        live.fetch_and(false, Ordering::SeqCst);

        if handshaked {
            err_cnt = 0;
        }

        if let Err(e) = res {
            let sleep_secs = 1 << err_cnt;
            error!("Protocol error: {e:?}. Reconnecting in {sleep_secs} seconds");
            err_cnt = core::cmp::min(err_cnt + 1, 5);
            sleep(Duration::from_secs(sleep_secs)).await;
        } else {
            break;
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct ClientConfig {
    #[arg(
        long,
        help = "Which servers to connect to in order to receive mining requests from",
        value_delimiter = ','
    )]
    pub miner_connect: Vec<SocketAddr>,
    #[cfg(not(feature = "force-tls"))]
    #[arg(long, help = "Use TLS for the miner")]
    miner_connect_tls: bool,
    #[arg(long, help = "Number of threads to mine with defaults to one less than the number of cpus available.", default_value = None)]
    pub num_threads: Option<u64>,
    #[arg(
        long,
        help = "Pin miner threads to given CPU cores. Format: sequence=starting_core, exact=core1,core2,core3, or performance"
    )]
    pub pin_threads: Option<PinThreads>,
    #[arg(
        long,
        help = "What's the client name to send in the porotocol"
    )]
    pub client_name: Option<String>,
    #[cfg(feature = "gpu")]
    #[arg(
        long,
        help = "Which miner threads to enable the GPU mining for. Format: miner1,miner2=gpuNum1,miner3=gpuNum2,miner5=gpuNum2:gpuNameFilter",
        value_delimiter = ','
    )]
    pub gpu_miners: Vec<GpuConfig>,
}

impl ClientConfig {
    pub fn num_threads(&self) -> u64 {
        self.num_threads.unwrap_or(1)
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

#[cfg(feature = "gpu")]
#[derive(Clone, Debug, Default)]
struct GpuConfig {
    miner: usize,
    gpu_index: usize,
    name_filter: Option<String>,
}

#[cfg(feature = "gpu")]
impl FromStr for GpuConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (miner, rest) = s.split_once("=").unwrap_or((s, ""));
        let (gpu_index, name_filter) = rest
            .split_once(":")
            .map(|(a, b)| (a, Some(b.to_string())))
            .unwrap_or((rest, None));

        let miner = miner
            .parse::<usize>()
            .map_err(|_| format!("Invalid miner id: {miner}"))?;

        let gpu_index = if gpu_index.is_empty() {
            0
        } else {
            gpu_index
                .parse::<usize>()
                .map_err(|_| format!("Invalid GPU index: {gpu_index}"))?
        };

        Ok(Self {
            miner,
            gpu_index,
            name_filter,
        })
    }
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

async fn start_mining_attempt(
    miner: &MinerHandle,
    server_extras: &[ServerExtras],
    requests: &mut BTreeMap<usize, (MiningData, usize, Instant, u32)>,
    //mining_data: tokio::sync::MutexGuard<'_, Option<MiningData>>,
    //nonce: Option<NounSlab>,
    //cancel_previous: bool,
) {
    let max_height = requests.values().map(|v| v.0.block_height).max().unwrap_or(0);
    gauge!(
        "nbx_miner_client_block_height",
    ).set(max_height as f64);

    let mut live_cnt = 0;

    let filtered = requests
        .iter()
        .filter(|(sid, _)| if server_extras[**sid].live.load(Ordering::SeqCst) { live_cnt += 1; true } else { false })
        .filter(|(_, v)| v.0.block_height == max_height)
        .min_by_key(|(_, v)| v.2);

    gauge!(
        "nbx_miner_client_live_servers",
    ).set(live_cnt as f64);

    let Some((target_sid, (mining_data, data_id, _, session_id))) = filtered else {
        return;
    };

    let mut rng = rand::thread_rng();
    let mut nonce_slab = NounSlab::<NockJammer>::new();
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
    let nonce = nonce_slab;

    debug!(
        "starting mining attempt on thread {:?} on header {:?} on block {} with nonce: {:?}",
        miner.id,
        tip5_hash_to_base58(*unsafe { mining_data.block_header.root() })
            .expect("Failed to convert block header to Base58"),
        mining_data.block_height,
        tip5_hash_to_base58(*unsafe { nonce.root() }).expect("Failed to convert nonce to Base58"),
    );
    let poke_slab = create_poke(mining_data, &nonce);
    miner.send_poke(poke_slab, *target_sid, *data_id, *session_id, false);
}

struct Miner {
    serf: SerfThread<SaveableCheckpoint>,
    id: usize,
    results: mpsc::Sender<MinerAttemptRes>,
    reqs: watch::Receiver<SyncMutex<Option<(NounSlab, usize, usize, u32)>>>,
}

impl Miner {
    pub async fn run(mut self) {

        let attempt_hist = histogram!(
            "nbx_miner_client_attempt_seconds",
            "miner_id" => self.id.to_string(),
        );
        let attempts_counter = counter!(
            "nbx_miner_client_attempts_count",
            "miner_id" => self.id.to_string(),
        );

        while self.reqs.changed().await.is_ok() {
            let Some((poke_slab, server_id, data_id, session_id)) = ({
                let mtx = self.reqs.borrow_and_update();
                let mut guard = mtx.lock().expect("Poisoned lock");
                guard.take()
            }) else {
                continue;
            };

            attempts_counter.increment(1);

            let start = Instant::now();
            let result = self
                .serf
                .poke(MiningWire::Candidate.to_wire(), poke_slab.clone())
                .await;

            let results = MinerAttemptRes {
                duration: start.elapsed(),
                id: self.id,
                server_id,
                data_id,
                slab_res: result,
                slab_inp: poke_slab,
                session_id,
            };

            attempt_hist.record(results.duration.as_secs_f64());

            if self.results.send(results).await.is_err() {
                break;
            }
        }
    }
}

struct MinerHandle {
    reqs: watch::Sender<SyncMutex<Option<(NounSlab, usize, usize, u32)>>>,
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
        results: mpsc::Sender<MinerAttemptRes>,
        #[cfg(feature = "gpu")] gpu_miner: Option<GpuConfig>,
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
            debug!("Pinning miner {id} to core {core_id}");
            serf.call_fn(move || gdt_cpus::pin_thread_to_core(core_id))
                .await
                .expect("Could not invoke core pinning")
                .expect("Could not pin the miner thread");
        }

        #[cfg(feature = "gpu")]
        if let Some(config) = gpu_miner {
            debug!("Initializing gpu {config:?}");
            serf.call_fn(move || {
                nbx_jetpack::gpu::init_gpu(config.name_filter.as_deref(), config.gpu_index)
            })
            .await
            .expect("Could not invoke gpu initialization");
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

    pub fn send_poke(
        &self,
        poke_slab: NounSlab,
        server_id: usize,
        data_id: usize,
        session_id: u32,
        cancel_previous: bool,
    ) {
        self.reqs.send_modify(|v| {
            let mut guard = v.lock().expect("Poisoned lock");
            *guard = Some((poke_slab, server_id, data_id, session_id));
            if cancel_previous {
                // Cancel while holding the guard to prevent the miner from racing to a stale request.
                self.cancel_current_poke();
            }
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
