use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Args;
use gdt_cpus::CoreType;
use nbx_jetpack::log::*;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::sleep;

use crate::metrics::gauge;
use crate::proto::{self, MiningAckOut, MiningDataOut, MiningResultIn};
use crate::shared::{tls_connect_wrap, TlsClientConfig};

pub struct ServerExtras {
    pub mining_res: mpsc::Sender<MiningResultIn>,
    pub live: Arc<AtomicBool>,
}

pub fn client_loops(
    miner_connect: Vec<SocketAddr>,
    tls: Option<TlsClientConfig>,
    client_name: &String,
    mining_tx: mpsc::Sender<MiningDataOut>,
    ack_tx: mpsc::Sender<MiningAckOut>,
    miner_metadata: Vec<BTreeMap<String, Arc<str>>>,
) -> (JoinSet<()>, Vec<ServerExtras>) {
    let mut client_tasks = JoinSet::new();
    let mut server_extras = vec![];
    let client_name = Arc::<str>::from(&(**client_name));

    for (i, a) in miner_connect.into_iter().enumerate() {
        let (tx, rx) = mpsc::channel(64);
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

    (client_tasks, server_extras)
}

async fn client_loop(
    addr: SocketAddr,
    tls: Option<TlsClientConfig>,
    server_id: usize,
    client_name: Arc<str>,
    live: Arc<AtomicBool>,
    mut results: mpsc::Receiver<MiningResultIn>,
    data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
    miner_metadata: Vec<BTreeMap<String, Arc<str>>>,
) {
    let mut err_cnt = 0;
    let server_name = addr.to_string();
    for i in 1.. {
        let stream = match tls_connect_wrap(TcpStream::connect(addr), tls).await {
            Ok(stream) => stream,
            Err(e) => {
                let sleep_secs = 1 << err_cnt;
                error!("Unable to connect to {addr}: {e:?}. Sleeping for {sleep_secs} seconds");
                sleep(Duration::from_secs(sleep_secs)).await;
                err_cnt = core::cmp::min(err_cnt + 1, 5);
                gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set(err_cnt as f64);
                continue;
            }
        };

        let mut handshaked = false;

        live.fetch_or(true, Ordering::SeqCst);
        let res = proto::client(
            stream,
            server_id,
            &server_name,
            client_name.clone(),
            &mut results,
            data.clone(),
            ack.clone(),
            miner_metadata.clone(),
            &mut handshaked,
        );

        #[cfg(feature = "stealthy")]
        let metrics_keepalive = std::future::pending::<()>();

        #[cfg(not(feature = "stealthy"))]
        let metrics_keepalive = async {
            loop {
                gauge!("nbx_miner_client_loop_connected_count", "server_id" => server_id.to_string()).set(i as f64);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        };

        let res = tokio::select! {
            v = res => v,
            _ = metrics_keepalive => unreachable!(),
        };

        live.fetch_and(false, Ordering::SeqCst);

        if handshaked {
            err_cnt = 0;
        }

        if let Err(e) = res {
            let sleep_secs = 1 << err_cnt;
            error!("Protocol error: {e:?}. Reconnecting in {sleep_secs} seconds");
            err_cnt = core::cmp::min(err_cnt + 1, 5);
            gauge!("nbx_miner_client_loop_connect_error_count", "server_id" => server_id.to_string()).set(err_cnt as f64);
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
    pub miner_connect_tls: bool,
    #[arg(long, help = "Number of threads to mine with defaults to one less than the number of cpus available.", default_value = None)]
    pub num_threads: Option<u64>,
    #[arg(
        long,
        help = "Pin miner threads to given CPU cores. Format: sequence=starting_core, exact=core1,core2,core3, or performance"
    )]
    pub pin_threads: Option<PinThreads>,
    #[arg(long, help = "What's the client name to send in the protocol")]
    pub client_name: Option<String>,
    #[cfg(not(feature = "force-send-only-targets"))]
    #[arg(long, help = "Whether to forward non-block proofs upstream")]
    pub forward_non_block: bool,
    #[cfg(feature = "gpu")]
    #[arg(
        long,
        help = "Which GPU's to use. Format: 1,2,3:gpuNameFilter",
        value_delimiter = ','
    )]
    pub gpus: Vec<GpuConfig>,
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
