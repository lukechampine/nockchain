use std::collections::{BTreeMap, HashMap};
use std::future::pending;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use clap::Args;
use either::Either;
use gdt_cpus::CoreType;
use nbx_jetpack::log::*;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use rustls::crypto::ring::default_provider;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::task::{Id, JoinSet};
use tokio::time::sleep;

use crate::device::Device;
use crate::metrics::gauge;
use crate::proto::{
    self, ClientDataWrite, MiningAckOut, MiningDataOut, MiningResultIn, Permissions,
};
use crate::shared::{tls_connect, TlsClientConfig};

#[derive(Default, Clone, Copy)]
pub struct SharedExtras {
    pub live: bool,
    pub session_id: u32,
    pub perms: Permissions,
}

pub struct ServerExtras {
    pub mining_res: mpsc::Sender<ClientDataWrite>,
    pub shared: Arc<SyncMutex<SharedExtras>>,
}

impl ServerExtras {
    pub fn shared(&self) -> SharedExtras {
        *self.shared.lock().unwrap()
    }
}

#[derive(Clone)]
struct ClientExtras {
    id: usize,
    mining_res: Arc<Mutex<mpsc::Receiver<ClientDataWrite>>>,
    shared: Arc<SyncMutex<SharedExtras>>,
}

struct PoolEntry {
    server_name: Option<String>,
    err_cnt: Arc<AtomicUsize>,
    spawn_cnt: usize,
    last_died: Instant,
    extras: Option<ClientExtras>,
    handle: Option<tokio::task::AbortHandle>,
}

impl PoolEntry {
    fn spawn_time(&self) -> Instant {
        self.last_died
            + Duration::from_secs(
                (1u64 << core::cmp::min(5, self.err_cnt.load(Ordering::Relaxed))) - 1,
            )
    }
}

async fn resolve_all(seeds: &[String]) -> std::io::Result<BTreeMap<SocketAddr, Option<String>>> {
    let mut out = BTreeMap::new();
    for s in seeds {
        if let Ok(sa) = s.parse::<SocketAddr>() {
            out.insert(sa, None);
        } else {
            for sa in tokio::net::lookup_host(s).await? {
                // Currently do not do ipv6
                // TODO: do ipv6, if we support.
                if sa.is_ipv4() {
                    out.insert(sa, Some(s.clone()));
                }
            }
        }
    }
    Ok(out)
}

pub fn client_loops(
    miner_connect: Vec<String>,
    num_concurrent_connections: usize,
    device: Device,
    mining_tx: mpsc::Sender<MiningDataOut>,
    ack_tx: mpsc::Sender<MiningAckOut>,
    jwt: Option<Arc<str>>,
) -> (JoinSet<()>, Vec<ServerExtras>) {
    #[cfg(feature = "jwt-auth-client")]
    let jwt = jwt.or_else(|| std::env::var("NBX_AUTH_JWT").ok().map(Arc::<str>::from));

    let _ = default_provider().install_default();

    let mut client_tasks = JoinSet::new();
    let mut server_extras = vec![];
    let mut client_extras = vec![];

    for id in 0..num_concurrent_connections {
        let (tx, rx) = mpsc::channel(64);
        let shared = Arc::new(SyncMutex::new(SharedExtras::default()));
        client_extras.push(ClientExtras {
            id,
            mining_res: Arc::new(Mutex::new(rx)),
            shared: shared.clone(),
        });
        server_extras.push(ServerExtras {
            mining_res: tx,
            shared,
        });
    }

    client_tasks.spawn(client_pool(
        miner_connect,
        device.into(),
        client_extras,
        mining_tx,
        ack_tx,
        jwt,
    ));

    (client_tasks, server_extras)
}

async fn client_pool(
    client_connect: Vec<String>,
    device: Arc<Device>,
    mut client_extras: Vec<ClientExtras>,
    mining_tx: mpsc::Sender<MiningDataOut>,
    ack_tx: mpsc::Sender<MiningAckOut>,
    jwt: Option<Arc<str>>,
) {
    let mut spawned: HashMap<Id, SocketAddr> = HashMap::new();
    let mut pool: HashMap<SocketAddr, PoolEntry> = HashMap::new();
    let mut tasks: JoinSet<()> = JoinSet::new();

    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut resolved: BTreeMap<SocketAddr, Option<String>> = BTreeMap::new();

    let tls_ip = Arc::new(TlsClientConfig::pinned_default());
    // TODO: feature flag to not force the server name
    let tls_dns = Arc::new(TlsClientConfig::forced_server_name(
        "pool-proxy.intra.nockbox.org".into(),
    ));

    loop {
        let mut backoff = None;
        while !client_extras.is_empty() {
            let mut candidates = pool
                .iter_mut()
                .filter(|(addr, e)| resolved.contains_key(addr) && e.handle.is_none())
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(_, e)| e.spawn_time());
            let Some((a, c)) = candidates.get_mut(0) else {
                break;
            };
            let now = Instant::now();
            let spawn_time = c.spawn_time();
            if spawn_time > now {
                debug!(
                    "{a} in exponential backoff for {:.02}s",
                    spawn_time.duration_since(now).as_secs_f64()
                );
                backoff = Some(spawn_time);
                break;
            }
            // Re-query again, because we want to randomize our choice
            candidates.retain(|v| v.1.spawn_time() <= now);
            candidates.shuffle(&mut thread_rng());
            // The retain call here already includes the first element, which matches the condition
            // due to the backoff check.
            let (a, c) = &mut candidates[0];

            // We just checked this
            let sn = resolved.get(a).unwrap().clone();
            let tls = if let Some(sn) = &sn {
                debug!("Connecting to {sn} on {a}");
                tls_dns.clone()
            } else {
                debug!("Connecting to {a}");
                tls_ip.clone()
            };
            // We know we are not empty
            let extras = client_extras.pop().unwrap();
            c.spawn_cnt += 1;
            let jh = tasks.spawn(client_conn(
                **a,
                tls,
                sn,
                device.clone(),
                jwt.clone(),
                extras.clone(),
                c.err_cnt.clone(),
                mining_tx.clone(),
                ack_tx.clone(),
            ));
            spawned.insert(jh.id(), **a);
            c.handle = Some(jh);
            c.extras = Some(extras);
        }

        // Disconnect one obselete connection if we are at capacity.
        if client_extras.is_empty() {
            for (id, v) in &spawned {
                if !resolved.contains_key(v) {
                    let id = *id;
                    let v = *v;
                    debug!("{v} no longer resolvable. Killing");
                    pool.get(&v).unwrap().handle.as_ref().unwrap().abort();
                    break;
                }
            }
        }

        tokio::select! {
            _ = async {
                if let Some(backoff) = backoff {
                    tokio::time::sleep_until(backoff.into()).await
                } else {
                    pending().await
                }
            } => {}
            _ = ticker.tick() => {
                resolved = resolve_all(&client_connect).await.unwrap_or_default();
                for (a, server_name) in resolved.iter() {
                    trace!("Resolved {a} on domain {server_name:?}");
                    pool.entry(*a).or_insert_with(|| PoolEntry {
                        server_name: server_name.clone(),
                        err_cnt: Arc::new(AtomicUsize::new(0)),
                        last_died: Instant::now(),
                        spawn_cnt: 0,
                        extras: None,
                        handle: None,
                    });
                }
            }
            Some(r) = tasks.join_next_with_id() => {
                let id = r.map(|v| v.0).unwrap_or_else(|e| e.id());
                let addr = spawned.remove(&id).unwrap();
                let e = pool.get_mut(&addr).unwrap();
                e.handle = None;
                e.last_died = Instant::now();
                let extras = e.extras.take().unwrap();
                *extras.shared.lock().unwrap() = Default::default();
                client_extras.push(extras);
            }
        }

        let to_remove = pool
            .iter()
            .inspect(|(a, e)| {
                gauge!("nbx_miner_client_loop_connected_count", "server_addr" => a.to_string(), "server_name" => e.server_name.clone().unwrap_or_default()).set(e.spawn_cnt as f64);
                gauge!("nbx_miner_client_loop_err_cnt", "server_addr" => a.to_string(), "server_name" => e.server_name.clone().unwrap_or_default()).set(e.err_cnt.load(Ordering::Relaxed) as f64);
            })
            .filter(|(addr, e)| !resolved.contains_key(addr) && e.handle.is_none())
            .map(|(addr, _)| *addr)
            .collect::<Vec<_>>();

        for a in to_remove {
            trace!("Cleaning up {a}");
            pool.remove(&a);
        }
    }
}

async fn client_conn(
    addr: SocketAddr,
    tls: Arc<TlsClientConfig>,
    server_name: Option<String>,
    device: Arc<Device>,
    jwt: Option<Arc<str>>,
    ClientExtras {
        id: server_id,
        mining_res,
        shared,
    }: ClientExtras,
    err_cnt: Arc<AtomicUsize>,
    data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
) {
    // This is the only place we access it, and the arc is handed exclusively.
    let mut results = mining_res.try_lock().unwrap();
    let server_proto_name = addr.to_string();

    let stream = match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(addr)).await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            error!("Unable to connect to {addr}: {e}.");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
        Err(_) => {
            error!("Timeout connecting to {addr}");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
    };

    let stream = match tokio::time::timeout(
        Duration::from_secs(10),
        tls_connect(stream, &tls, server_name.clone()),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            error!("Unable to establish TLS on {addr}: {e}.");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
        Err(_) => {
            error!("Timeout establishing TLS on {addr}");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
    };

    let handshake = match tokio::time::timeout(
        Duration::from_secs(20),
        proto::client_handshake(stream, server_id, &server_proto_name, device, jwt.clone()),
    )
    .await
    {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            error!("Handshake failed: {e}.");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
        Err(_) => {
            error!("Handshake timeout.");
            let c = err_cnt.fetch_add(1, Ordering::Relaxed);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set((c + 1) as f64);
            return;
        }
    };

    {
        let mut shared = shared.lock().unwrap();
        shared.live = true;
        shared.session_id = handshake.session_id;
        shared.perms = handshake.perms;
    }
    err_cnt.store(0, Ordering::Relaxed);

    let res = proto::client(handshake, &mut results, data.clone(), ack.clone()).await;

    shared.lock().unwrap().live = false;

    if let Err(e) = res {
        error!("Connection finished: {e}.");
        let c = err_cnt.fetch_add(1, Ordering::Relaxed);
        gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string())
            .set((c + 1) as f64);
    }
}

#[derive(Args, Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    #[arg(
        long,
        help = "Which servers to connect to in order to receive mining requests from",
        value_delimiter = ','
    )]
    pub miner_connect: Vec<String>,
    #[cfg(not(feature = "force-tls"))]
    #[arg(long, help = "Use TLS for the miner")]
    pub miner_connect_tls: bool,
    #[arg(
        long,
        help = "How many concurrent connections to maintain",
        default_value = "3"
    )]
    pub miner_num_concurrent_connections: usize,
    #[arg(long, help = "Number of threads to mine with defaults to one less than the number of cpus available.", default_value = None)]
    pub num_threads: Option<u64>,
    #[arg(
        long,
        help = "Pin miner threads to given CPU cores. Format: sequence=starting_core, exact=core1,core2,core3, or performance"
    )]
    pub pin_threads: Option<PinThreads>,
    #[arg(
        long,
        help = "What's the client name to send in the protocol. Affects machine ID."
    )]
    pub client_name: Option<String>,
    #[cfg(feature = "gpu")]
    #[arg(
        long,
        help = "Which GPU's to use. Format: 1,2,3:gpuNameFilter",
        value_delimiter = ','
    )]
    pub gpus: Vec<GpuConfig>,
    #[cfg(feature = "jwt-auth-client")]
    #[arg(
        long,
        help = "JWT to use in order to authenticate to servers. Overrides NBX_AUTH_JWT environment variable."
    )]
    pub miner_auth_jwt: Option<Arc<str>>,
}

impl ClientConfig {
    pub fn num_threads(&self) -> u64 {
        self.num_threads.unwrap_or(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GpuConfig {
    pub gpu_index: usize,
    pub name_filter: Option<String>,
}

#[cfg(feature = "gpu")]
impl FromStr for GpuConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (gpu_index, name_filter) = s
            .split_once(":")
            .map(|(a, b)| (a, Some(b.to_string())))
            .unwrap_or((s, None));

        let gpu_index = if gpu_index.is_empty() {
            0
        } else {
            gpu_index
                .parse::<usize>()
                .map_err(|_| format!("Invalid GPU index: {gpu_index}"))?
        };

        Ok(Self {
            gpu_index,
            name_filter,
        })
    }
}
