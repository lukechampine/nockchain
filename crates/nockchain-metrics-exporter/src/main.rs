#![allow(clippy::doc_overindented_list_items)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use ibig::UBig;
use metrics::gauge;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::MetricKindMask;
use nockapp::driver::*;
use nockapp::kernel::boot::{default_boot_cli, init_default_tracing};
use nockapp::noun::slab::NounSlab;
use nockapp::wire::WireTag;
use nockapp::{Bytes, NockAppError, NockAppExit, Noun};
use nockapp_grpc::services::private_nockapp::PrivateNockAppGrpcClient;
use nockapp_grpc_proto::pb::common::v1::Wire as GrpcWire;
use nockvm::noun::{IndirectAtom, D, T};
use nockvm_macros::tas;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::time::{interval, timeout, MissedTickBehavior};
use tracing::{debug, error, info, trace};

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
    #[arg(
        short,
        long,
        help = "gRPC server address (e.g., http://127.0.0.1:5555)",
        default_value = "http://127.0.0.1:5555"
    )]
    grpc_address: String,
    #[arg(
        short,
        long,
        default_value = "1",
        help = "seconds to refresh the metrics"
    )]
    refresh_interval: usize,
}

struct GrpcHandle {
    client: PrivateNockAppGrpcClient,
    pid: i32,
}

impl GrpcHandle {
    pub async fn new(address: &str) -> Result<Self, NockAppError> {
        let client = PrivateNockAppGrpcClient::connect(address)
            .await
            .map_err(|e| {
                NockAppError::OtherError(format!("Failed to connect to gRPC server: {}", e))
            })?;

        Ok(Self { client, pid: 0 })
    }

    pub async fn peek(&mut self, path: &[&str]) -> Result<NounSlab, NockAppError> {
        let path_strings: Vec<String> = path.iter().map(|s| s.to_string()).collect();

        let jam_bytes = self
            .client
            .peek(self.pid, path_strings)
            .await
            .map_err(|e| NockAppError::OtherError(format!("gRPC peek failed: {}", e)))?;

        let mut slab = NounSlab::new();
        let noun = slab.cue_into(Bytes::from(jam_bytes))?;
        slab.set_root(noun);

        Ok(slab)
    }
}

fn target_to_difficulty(target: UBig) -> f64 {
    let p = UBig::from(0xffffffff00000001u64);
    let p1: UBig = p.clone() - 1;
    let mut max_target = p1.clone();
    for i in 1..=4 {
        max_target += p1.clone() * p.pow(i);
    }

    (max_target / target).to_f64()
}

fn parse_bn(mut n: Noun) -> UBig {
    let mut cnt = 0;
    let mut val = UBig::default();

    while let Ok(c) = n.as_cell() {
        let Ok(h) = c.head().as_atom().and_then(|v| v.as_direct()) else {
            // TODO: throw error?
            break;
        };
        let v = h.data();
        if cnt > 0 {
            let v = UBig::from(v);
            let v2 = v.clone() << (32 * (cnt - 1));
            val += v2;
        }
        cnt += 1;
        n = c.tail();
    }

    val
}

struct Exporter {
    grpc_handle: GrpcHandle,
    id: String,
}

impl Exporter {
    pub async fn new(grpc_address: &str, id: String) -> Result<Self, NockAppError> {
        let grpc_handle = GrpcHandle::new(grpc_address).await?;

        Ok(Self { grpc_handle, id })
    }

    pub async fn update(&mut self) -> Result<(), NockAppError> {
        let poke = match timeout(
            Duration::from_secs(10),
            self.grpc_handle.peek(&["heavy-summary"]),
        )
        .await
        {
            Ok(Ok(p)) => p,
            Err(_) => {
                error!("Timeout sending gRPC peek");
                return Err(NockAppError::Timeout);
            }
            Ok(Err(e)) => {
                error!("Unable to send gRPC peek: {:?}", e);
                return Err(NockAppError::UnexpectedResult);
            }
        };
        let eff = unsafe { poke.root() };
        let [_, _, _, _, summary] = pull_args(*eff)?;
        let [_digest, timestamp, epoch_counter, target, accumulated_work, height, _parent] =
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
        let target = parse_bn(target);
        let difficulty = target_to_difficulty(target);
        let accumulated_work = parse_bn(accumulated_work);
        let accumulated_work = accumulated_work.to_f64();

        gauge!("nockchain_block_timestamp").set(timestamp);
        gauge!("nockchain_block_height").set(height);
        gauge!("nockchain_block_epoch_counter").set(epoch_counter);
        gauge!("nockchain_block_difficulty").set(difficulty);
        gauge!("nockchain_block_accumulated_work").set(accumulated_work);

        Ok(())
    }
}

struct RetryExporter {
    grpc_address: String,
    exporter: Option<Exporter>,
    retry_instant: Instant,
    id: String,
}

impl RetryExporter {
    pub fn new(grpc_address: String) -> Self {
        // TODO: pull miner ID out
        let id = grpc_address.clone();

        Self {
            grpc_address,
            exporter: None,
            retry_instant: Instant::now(),
            id,
        }
    }

    pub fn disconnect(&mut self) {
        self.exporter = None;
    }

    pub async fn acquire(&mut self) -> Option<&mut Exporter> {
        if let Some(mut exporter) = self.exporter.take() {
            self.exporter = Some(exporter);
            return self.exporter.as_mut();
        }

        if self.retry_instant <= Instant::now() {
            match Exporter::new(&self.grpc_address, self.id.clone()).await {
                Ok(exporter) => {
                    self.exporter = Some(exporter);
                }
                Err(e) => {
                    trace!("Unable to connect to gRPC server: {:?}. Retrying...", e);
                    self.retry_instant = Instant::now() + Duration::from_secs(5);
                }
            }
        }

        self.exporter.as_mut()
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

    let mut exporter = RetryExporter::new(cli.grpc_address);
    let mut interval = interval(Duration::from_secs(cli.refresh_interval as u64));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut error_cnt = 0;

    loop {
        if let Some(e) = exporter.acquire().await {
            match e.update().await {
                Err(NockAppError::Timeout) => {
                    exporter.disconnect();
                }
                Err(e) => {
                    debug!("Exporter error: {e:?}");
                    error_cnt += 1;
                    if error_cnt > 5 {
                        error_cnt = 1;
                        exporter.disconnect();
                    }
                }
                _ => error_cnt = 0,
            }
        }

        interval.tick().await;
    }
}
