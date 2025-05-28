#![allow(clippy::doc_overindented_list_items)]

use futures::stream::FuturesUnordered;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use nockapp::kernel::boot::{default_boot_cli, init_default_tracing};
use nockvm_macros::tas;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{interval, timeout, MissedTickBehavior};

use clap::Parser;
use futures::StreamExt;
use metrics::{counter, gauge};
use nockapp::{NockAppError, NockAppExit, Noun};
use nockvm::noun::{IndirectAtom, D, T};
use sha3::{Digest, Sha3_256};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tracing::{debug, error, info, trace};

use nockapp::driver::*;
use nockapp::noun::slab::NounSlab;
use nockapp::wire::WireTag;

fn pull_args<const N: usize>(mut inp: Noun) -> Result<[Noun; N], NockAppError> {
    let mut cnt = 0;
    let mut ret = [(); N].map(|_| {
        cnt += 1;
        if cnt == N {
            Ok(inp)
        } else {
            let c = inp.as_cell()?;
            inp = c.tail();
            Ok(c.head())
        }
    });
    if let Some(e) = ret.iter_mut().filter(|v| v.is_err()).next() {
        let n = core::mem::replace(e, Ok(D(0)));
        return Err(n.unwrap_err());
    }
    Ok(ret.map(|v| v.unwrap()))
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct MetricsCli {
    #[arg(short, long, default_value = "127.0.0.1:9089")]
    bind: String,
    #[arg(short, long, help = "paths to nockchain.sock")]
    socket: Vec<String>,
    #[arg(
        short,
        long,
        default_value = "1",
        help = "seconds to refresh the metrics"
    )]
    refresh_interval: usize,
}

struct NpcHandler {
    io_receiver: Receiver<IOAction>,
    effect_sender: Arc<broadcast::Sender<NounSlab>>,
    wires: HashMap<u64, oneshot::Sender<NounSlab>>,
    req_recv: Receiver<(NounSlab, oneshot::Sender<NounSlab>)>,
    pid: u64,
}

impl NpcHandler {
    pub fn new(
        io_receiver: Receiver<IOAction>,
        effect_sender: Arc<broadcast::Sender<NounSlab>>,
    ) -> (Self, NpcHandle) {
        let (req_send, req_recv) = channel(4);

        (
            Self {
                io_receiver,
                effect_sender,
                wires: Default::default(),
                req_recv,
                pid: 0,
            },
            NpcHandle(req_send),
        )
    }

    async fn handle_req(
        &mut self,
        mut slab: NounSlab,
        tx: oneshot::Sender<NounSlab>,
    ) -> Result<(), NockAppError> {
        let pid = self.pid;
        self.pid += 1;
        let io = *unsafe { slab.root() };
        let eff = T(&mut slab, &[D(tas!(b"npc")), D(pid), io]);
        slab.set_root(eff);
        if self.effect_sender.send(slab).is_err() {
            return Err(NockAppError::UnexpectedResult);
        }
        self.wires.insert(pid, tx);
        Ok(())
    }

    async fn handle_io(&mut self, io: IOAction) -> Result<(), NockAppError> {
        let IOAction::Poke {
            wire,
            mut poke,
            ack_channel,
        } = io
        else {
            error!("Invalid action");
            return Err(NockAppError::UnexpectedResult);
        };

        if wire.source != "npc" {
            error!("Invalid wire source: {}", wire.source);
            let _ = ack_channel.send(PokeResult::Nack);
            return Err(NockAppError::UnexpectedResult);
        }

        let &[_, WireTag::Direct(pid)] = &wire.tags[..] else {
            error!("Invalid wire tags: {:?}", wire.tags);
            let _ = ack_channel.send(PokeResult::Nack);
            return Err(NockAppError::UnexpectedResult);
        };

        let root = unsafe { poke.root() };
        let [_, _, v] = pull_args(*root)?;
        poke.set_root(v);

        let Some(tx) = self.wires.remove(&pid) else {
            error!("Wire not found by pid: {pid}");
            let _ = ack_channel.send(PokeResult::Nack);
            return Err(NockAppError::UnexpectedResult);
        };

        let _ = ack_channel.send(PokeResult::Ack);
        let _ = tx.send(poke);

        Ok(())
    }

    pub async fn serve(mut self) {
        loop {
            tokio::select! {
                req = self.req_recv.recv() => {
                    let Some((slab, tx)) = req else { break };
                    if self.handle_req(slab, tx).await.is_err() {
                        break;
                    }
                },
                io = self.io_receiver.recv() => {
                    let Some(io) = io else { break };
                    let _ = self.handle_io(io).await;
                }
            }
        }
    }
}

struct NpcHandle(Sender<(NounSlab, oneshot::Sender<NounSlab>)>);

impl NpcHandle {
    pub async fn peek(&self, path: &[&str]) -> Result<NounSlab, NockAppError> {
        let mut slab = NounSlab::new();
        let mut req = vec![D(tas!(b"peek"))];
        req.extend(path.iter().map(|p| {
            unsafe { IndirectAtom::new_raw_bytes_ref(&mut slab, str::as_bytes(p)) }.as_noun()
        }));
        req.push(D(0));
        let req = T(&mut slab, &req);
        slab.set_root(req);

        let (tx, rx) = oneshot::channel();
        if self.0.send((slab, tx)).await.is_err() {
            error!("Unable to send poke");
            return Err(NockAppError::UnexpectedResult);
        }
        let ret = rx.await?;

        let eff = unsafe { ret.root() };

        Ok(ret)
    }
}

struct Exporter {
    npc: NpcHandle,
    npc_handler: tokio::task::JoinHandle<()>,
    npc_client: tokio::task::JoinHandle<Result<(), NockAppError>>,
    id: String,
}

impl Exporter {
    pub async fn new(nockchain_socket: impl AsRef<Path>, id: String) -> Result<Self, NockAppError> {
        let (io_sender, io_receiver) = mpsc::channel(1);
        let (tx, rx) = broadcast::channel(1);
        let effect_sender = Arc::new(tx);
        let effect_receiver = Mutex::new(rx);

        let handle = NockAppHandle {
            io_sender,
            effect_sender,
            effect_receiver,
            exit: NockAppExit::new().0,
        };

        let (npc_handler, npc) = NpcHandler::new(io_receiver, handle.effect_sender.clone());
        let npc_handler = tokio::spawn(npc_handler.serve());

        let socket_path = nockchain_socket;

        let stream = UnixStream::connect(socket_path.as_ref())
            .await
            .map_err(|e| {
                eprintln!(
                    "Failed to connect to nockchain NPC socket at {:?}: {}\n\
                 This could mean:\n\
                 1. Nockchain is not running\n\
                 2. The socket path is incorrect\n\
                 3. The socket file exists but is stale (try removing it)\n\
                 4. Insufficient permissions to access the socket",
                    socket_path.as_ref(),
                    e
                );
                NockAppError::IoError(e)
            })?;

        info!(
            "Connected to nockchain NPC socket at {:?}",
            socket_path.as_ref()
        );
        let npc_client = tokio::spawn(nockapp::npc_client_driver(stream)(handle));

        Ok(Self {
            npc,
            npc_handler,
            npc_client,
            id,
        })
    }

    pub async fn update(&self) -> Result<(), NockAppError> {
        let poke = match timeout(Duration::from_secs(10), self.npc.peek(&["heavy-summary"])).await {
            Ok(Ok(p)) => p,
            Err(_) => {
                error!("Timeout sending poke");
                return Err(NockAppError::Timeout);
            }
            _ => {
                error!("Unable to send poke");
                return Err(NockAppError::UnexpectedResult);
            }
        };
        let eff = unsafe { poke.root() };
        let [_, _, _, _, summary] = pull_args(*eff)?;
        let [_digest, timestamp, epoch_counter, _target, _accumulated_work, height, _parent] =
            pull_args(summary)?;

        // TODO: unix timestamp conversion
        let timestamp = timestamp.as_atom()?;
        let dtimestamp = timestamp.as_direct().map(|v| v.data());
        let itimestamp = timestamp.as_indirect();
        let itimestamp = itimestamp.as_ref().map(|v| v.as_slice()[0]);
        let timestamp = dtimestamp.unwrap_or_else(|_| itimestamp.unwrap());
        let timestamp = timestamp as f64;

        let height = height.as_direct()?.data() as f64;
        let epoch_counter = epoch_counter.as_direct()?.data() as f64;

        gauge!("nockchain_block_timestamp", "path-id" => self.id.clone()).set(timestamp);
        gauge!("nockchain_block_height", "path-id" => self.id.clone()).set(height);
        gauge!("nockchain_block_epoch_counter", "path-id" => self.id.clone()).set(epoch_counter);

        Ok(())
    }
}

struct RetryExporter {
    socket_path: String,
    exporter: Option<Exporter>,
    retry_instant: Instant,
    id: String,
}

impl RetryExporter {
    pub fn new(socket_path: String) -> Self {
        // TODO: pull miner ID out
        let mut hasher = Sha3_256::new();
        hasher.update(socket_path.as_bytes());
        let res = hasher.finalize();
        let id = hex::encode(&res[..8]);

        Self {
            socket_path,
            exporter: None,
            retry_instant: Instant::now(),
            id,
        }
    }

    pub fn disconnect(&mut self) {
        self.exporter = None;
    }

    pub async fn acquire(&mut self) -> Option<&Exporter> {
        if let Some(exporter) = self.exporter.take() {
            if exporter.npc_client.is_finished() || exporter.npc_handler.is_finished() {
                debug!(
                    "Nockchain at {} has died. Reconnecting...",
                    self.socket_path
                );
                core::mem::drop(exporter);
                self.exporter = None;
            } else {
                self.exporter = Some(exporter);
            }
        }

        if self.exporter.is_none() && self.retry_instant <= Instant::now() {
            if let Ok(exporter) = Exporter::new(&self.socket_path, self.id.clone()).await {
                self.exporter = Some(exporter);
            } else {
                trace!("Unable to collect to node. Retrying...");
                self.retry_instant = Instant::now() + Duration::from_secs(5);
            }
        }

        self.exporter.as_ref()
    }
}

#[tokio::main]
async fn main() -> Result<(), NockAppError> {
    init_default_tracing(&default_boot_cli(false));

    let cli = MetricsCli::parse();

    let Ok(bind) = cli.bind.parse::<SocketAddr>() else {
        return Err(NockAppError::UnexpectedResult);
    };

    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(bind)
        .idle_timeout(MetricKindMask::GAUGE, Some(Duration::from_secs(60)))
        .install()
    {
        error!("{e:?}");
        return Err(NockAppError::UnexpectedResult);
    }

    let mut exporters = cli
        .socket
        .into_iter()
        .map(RetryExporter::new)
        .collect::<Vec<_>>();

    let mut interval = interval(Duration::from_secs(cli.refresh_interval as u64));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        exporters
            .iter_mut()
            .map(|v| async move {
                let Some(e) = v.acquire().await else {
                    return Ok(());
                };

                match e.update().await {
                    Err(NockAppError::Timeout) => {
                        v.disconnect();
                        Ok(())
                    }
                    v => v,
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;
        interval.tick().await;
    }
}
