use core::pin::pin;
use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use bincode::{Decode, Encode};
use futures::{Stream, StreamExt};
use nockapp::noun::slab::NounSlab;
use rand::random;
use strum::FromRepr;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};
use nbx_jetpack::log::*;
use tokio::task::JoinSet;
use zkvm_jetpack::form::Belt;
use crate::metrics::{counter, gauge, histogram};

use crate::shared;

pub const PROTOCOL: u32 = 5;
pub const CLIENT_NAME_MAX_LENGTH: usize = 16;
pub const RECENTLY_EXPIRED_DURATION: Duration = Duration::from_secs(10);

pub fn client_name_valid(client_name: &str) -> bool {
    client_name.len() <= CLIENT_NAME_MAX_LENGTH && client_name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct Hello {
    protocol: u32,
    nonce: u32,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct SetMinerMetadata {
    miners: Vec<BTreeMap<String, Arc<str>>>,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct MiningData {
    pub data_id: u32,
    pub block_header: Vec<u8>,
    pub version: Vec<u8>,
    pub target: Vec<u8>,
    pub pow_len: u64,
    pub block_height: u64,
    pub fixed_nonce_atoms: Vec<u64>,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct MiningDatas {
    pub expire: Vec<u32>,
    pub new_datas: Vec<MiningData>,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct MiningResult {
    pub data_id: u32,
    pub miner_id: u32,
    pub attempt_millis: u32,
    pub gpu_enqueue_millis: u32,
    pub gpu_submit_millis: u32,
    pub gpu_wait_millis: u32,
    pub target_hit: bool,
    pub poke: Option<Vec<u8>>,
    pub effect: Option<Vec<u8>>,
}

pub struct MiningResultIn {
    pub data_id: u32,
    pub session_id: u32,
    pub data: shared::MiningResult,
}

pub struct MiningResultOut {
    pub miner_metadata: Arc<BTreeMap<String, Arc<str>>>,
    pub client_id: usize,
    pub data: shared::MiningResult,
    pub in_data: Arc<shared::MiningData>,
}

pub struct MiningDataOut {
    pub server_id: usize,
    pub session_id: u32,
    pub expire: Vec<u32>,
    pub new_datas: Vec<(u32, shared::MiningData)>,
}

pub struct MiningAckOut {
    pub server_id: usize,
    pub miner_id: usize,
    pub data_id: u32,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
pub enum MinerResponse {
    METADATA = 0,
    RESULT = 1,
}

fn cue(d: Vec<u8>) -> NounSlab {
    let mut slab = NounSlab::new();
    let noun = slab.cue_into(d.into()).unwrap();
    slab.set_root(noun);
    slab
}

async fn binsend(mut stream: impl AsyncWrite + Unpin, target_name: Arc<str>, msg_type: &'static str, d: impl Encode) -> io::Result<()> {
    let t = Instant::now();
    let d = bincode::encode_to_vec(d, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;
    stream.write_u32_le(d.len() as _).await?;
    stream.write_all(&d).await?;
    stream.flush().await?;
    histogram!(
        "nbx_miner_binsend_seconds",
        "target_name" => target_name,
        "msg_type" => msg_type,
    ).record(t.elapsed().as_secs_f64());
    Ok(())
}

async fn binrecv<T: Decode<()>>(mut stream: impl AsyncRead + Unpin, target_name: Arc<str>, msg_type: &'static str) -> io::Result<T> {
    let len = stream.read_u32_le().await?;

    let t = Instant::now();

    // 16MB sanity limit
    if len > 0x1000000 {
        return Err(io::ErrorKind::OutOfMemory.into());
    }

    let mut buf = vec![0; len as usize];
    stream.read_exact(&mut buf).await?;
    let (res, _) = bincode::decode_from_slice(&buf, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;

    histogram!(
        "nbx_miner_binrecv_seconds",
        "target_name" => target_name,
        "msg_type" => msg_type,
    ).record(t.elapsed().as_secs_f64());

    Ok(res)
}

pub async fn client<S: AsyncRead + AsyncWrite>(
    stream: S,
    server_id: usize,
    server_name: &str,
    client_name: String,
    mining_out: &mut mpsc::Receiver<MiningResultIn>,
    mining_data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
    metadata: Vec<BTreeMap<String, Arc<str>>>,
    handshaked: &mut bool,
) -> io::Result<()> {
    let stream = pin!(stream);

    let (mut read, mut write) = split(stream);

    let server_name: Arc<str> = server_name.into();
    let server_id_str: Arc<str> = Arc::from(&*server_id.to_string());

    // Initial handshake
    let nonce = random::<u32>();
    binsend(
        &mut write,
        server_name.clone(),
        "hello",
        Hello {
            protocol: PROTOCOL,
            nonce,
        },
    )
    .await?;
    let resp: Hello = binrecv(&mut read, server_name.clone(), "hello").await?;
    if resp.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    if resp.nonce != nonce + 1 {
        return Err(io::ErrorKind::BrokenPipe.into());
    }

    binsend(
        &mut write,
        server_name.clone(),
        "client_name",
        client_name
    ).await?;

    *handshaked = true;

    #[cfg(feature = "stealthy")]
    let channel_mon = std::future::pending::<()>();

    #[cfg(not(feature = "stealthy"))]
    let channel_mon = async {
        loop {
            gauge!(
                "nbx_miner_proto_client_channel_capacity",
                "channel_name" => "mining_data",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            ).set(mining_data.capacity() as f64);
            gauge!(
                "nbx_miner_proto_client_channel_capacity",
                "channel_name" => "ack",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            ).set(ack.capacity() as f64);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };

    let receiver = async {
        loop {
            let datas: MiningDatas = binrecv(&mut read, server_name.clone(), "mining_data").await?;
            gauge!(
                "nbx_miner_proto_client_block_height",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            ).set(datas.new_datas.iter().map(|v| v.block_height).max().unwrap_or_default() as f64);
            if mining_data
                .send(MiningDataOut {
                    server_id,
                    session_id: nonce,
                    expire: datas.expire,
                    new_datas: datas.new_datas.into_iter().map(|data| (
                        data.data_id,
                        shared::MiningData {
                            block_header: cue(data.block_header),
                            version: cue(data.version),
                            target: cue(data.target),
                            pow_len: data.pow_len,
                            block_height: data.block_height,
                            fixed_nonce_atoms: data.fixed_nonce_atoms.into_iter().map(Belt).collect(),
                    })).collect::<Vec<_>>(),
                })
                .await
                .is_err()
            {
                break io::Result::Ok(());
            }
        }
    };

    let sender = async {
        write.write_u8(MinerResponse::METADATA as _).await?;
        binsend(
            &mut write,
            server_name.clone(),
            "miner_metadata",
            SetMinerMetadata { miners: metadata }
        ).await?;

        while let Some(MiningResultIn {
            data_id,
            session_id,
            data,
        }) = mining_out.recv().await
        {
            let miner_id: Arc<str> = Arc::from(&*data.miner_id.to_string());
            // Broadcast may contain previous session's datapoints. Skip them.
            if session_id != nonce {
                counter!(
                    "nbx_miner_proto_client_session_id_mismatch_count",
                    "server_name" => server_name.clone(),
                    "server_id" => server_id_str.clone(),
                    "miner_id" => miner_id.clone(),
                ).increment(1);
                continue;
            }
            let rdata = MiningResult {
                data_id,
                miner_id: data.miner_id as u32,
                attempt_millis: data.attempt_millis,
                gpu_enqueue_millis: data.gpu_enqueue_millis,
                gpu_submit_millis: data.gpu_submit_millis,
                gpu_wait_millis: data.gpu_wait_millis,
                target_hit: data.target_hit,
                poke: data.poke.as_ref().map(NounSlab::jam).map(Vec::from),
                effect: data.effect.as_ref().map(NounSlab::jam).map(Vec::from),
            };
            gauge!(
                "nbx_miner_proto_client_data_id",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
                "miner_id" => miner_id.clone(),
            ).set(rdata.data_id as f64);
            write.write_u8(MinerResponse::RESULT as _).await?;
            binsend(
                &mut write,
                server_name.clone(),
                "mining_result",
                rdata
            ).await?;
            let _ = ack
                .send(MiningAckOut {
                    server_id,
                    miner_id: data.miner_id,
                    data_id,
                })
                .await;
        }
        io::Result::Ok(())
    };

    tokio::select! {
        _ = channel_mon => unreachable!(),
        v = sender => v,
        v = receiver => v,
    }
}

pub async fn server<S: AsyncRead + AsyncWrite>(
    stream: S,
    mining_data: impl Stream<Item = (Arc<shared::MiningData>, Arc<OnceLock<Instant>>)>,
    client_id: usize,
    client_cn: Option<Arc<str>>,
    results_out: mpsc::Sender<MiningResultOut>,
) -> io::Result<()> {
    let stream = pin!(stream);
    let mut mining_data = pin!(mining_data);

    let (mut read, mut write) = split(stream);

    // Initial handshake
    let mut req: Hello = binrecv(&mut read, "".into(), "hello").await?;
    if req.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    req.nonce += 1;

    let client_cn = client_cn.unwrap_or_default();

    binsend(
        &mut write,
        client_cn.clone(),
        "hello",
        req
    ).await?;

    let client_name: String = binrecv(&mut read, client_cn.clone(), "client_name").await?;

    if !client_name_valid(&client_name) {
        return Err(io::ErrorKind::InvalidData.into());
    }

    let client_id_str: Arc<str> = Arc::from(&*client_id.to_string());

    debug!("Client ID {client_id} with CN {client_cn} joined with name '{client_name}'");

    let client_name: Arc<str> = Arc::from(&*format!("{client_cn}-{client_name}"));

    #[derive(Default)]
    struct DataTracker {
        data_id: u32,
        data_map: BTreeMap<u32, (Arc<shared::MiningData>, Arc<OnceLock<Instant>>)>,
        recently_expired_map: BTreeMap<u32, (Arc<shared::MiningData>, Instant)>,
    }

    impl DataTracker {
        fn add_data(&mut self, data: Arc<shared::MiningData>, expire: Arc<OnceLock<Instant>>) -> u32 {
            let data_id = self.data_id;
            self.data_id = self.data_id.wrapping_add(1);
            self.data_map.insert(data_id, (data, expire));
            data_id
        }

        fn remove_recently_expired(&mut self) {
            // Retain only for last 10 seconds, as to give enough time for shares to be submitted.
            self.recently_expired_map.retain(|_, v| v.1.elapsed() < RECENTLY_EXPIRED_DURATION);
        }

        fn collect_expired(&mut self) -> Vec<u32> {
            self.remove_recently_expired();
            let expired = self.data_map.iter().filter(|(_, (_, v))| v.get().is_some()).map(|(v, _)| *v).collect::<Vec<_>>();
            for i in &expired {
                let d = self.data_map.remove(&i).unwrap();
                self.recently_expired_map.insert(*i, (d.0, *d.1.get().unwrap()));
            }
            expired
        }

        fn valid_data(&mut self, data_id: u32) -> Option<Arc<shared::MiningData>> {
            self.remove_recently_expired();
            self.data_map
                .get(&data_id)
                .and_then(|(v, e)| {
                    if e.get().filter(|v| v.elapsed() >= RECENTLY_EXPIRED_DURATION).is_none() {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| self.recently_expired_map.get(&data_id).map(|(v, _)| v.clone()))
        }
    }

    let tracker = Mutex::new(DataTracker::default());

    #[cfg(feature = "stealthy")]
    let channel_mon = std::future::pending::<()>();

    #[cfg(not(feature = "stealthy"))]
    let channel_mon = async {
        loop {
            gauge!(
                "nbx_miner_proto_server_channel_capacity",
                "channel_name" => "results_out",
                "client_name" => client_name.clone(),
                "client_id" => client_id_str.clone(),
            ).set(results_out.capacity() as f64);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    let sender = async {
        let mut expiries = JoinSet::new();

        loop {
            let mut set_data = MiningDatas {
                expire: vec![],
                new_datas: vec![],
            };

            let mut guard = tokio::select! {
                d = mining_data.next() => {
                    let Some((data, expiration)) = d else {
                        break;
                    };
                    let mut guard = tracker.lock().await;

                    if expiration.get().is_none() {
                        gauge!(
                            "nbx_miner_proto_server_block_height",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).set(data.block_height as f64);

                        let data_id = guard.add_data(data.clone(), expiration.clone());
                        expiries.spawn(async move {
                            while expiration.get().is_none() {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        });
                        // We cannot have all 5 atoms fixed
                        assert!(data.fixed_nonce_atoms.len() < 5);
                        set_data.new_datas.push(MiningData {
                            data_id,
                            block_header: data.block_header.jam().into(),
                            version: data.version.jam().into(),
                            target: data.target.jam().into(),
                            pow_len: data.pow_len,
                            block_height: data.block_height,
                            fixed_nonce_atoms: data.fixed_nonce_atoms.iter().map(|v| v.0).collect(),
                        });
                    }
                    guard
                }
                Some(_) = expiries.join_next() => {
                    while expiries.try_join_next().is_some() {}
                    tracker.lock().await
                }
            };

            set_data.expire = guard.collect_expired();
            core::mem::drop(guard);

            binsend(
                &mut write,
                client_name.clone(),
                "mining_data",
                set_data
            ).await?;
            counter!(
                "nbx_miner_proto_server_set_data_count",
                "client_id" => client_id_str.clone(),
                "client_name" => client_name.clone(),
            ).increment(1);
        }

        io::Result::Ok(())
    };

    let receiver = async {
        let miner = Arc::new(BTreeMap::new());

        while let Ok(cmd) = read.read_u8().await {
            let Some(cmd) = MinerResponse::from_repr(cmd) else {
                error!("Invalid cmd: {cmd:x}. Exiting");
                return Err(io::ErrorKind::InvalidData.into());
            };
            match cmd {
                MinerResponse::METADATA => {
                    // TODO: remove, or change to fixed metadata
                    let _: SetMinerMetadata = binrecv(&mut read, client_name.clone(), "miner_metadata").await?;
                }
                MinerResponse::RESULT => {
                    let res: MiningResult = binrecv(&mut read, client_name.clone(), "mining_result").await?;

                    let mut guard = tracker.lock().await;
                    let Some(data) = guard.valid_data(res.data_id) else {
                        trace!("Unable to find data with ID {} ({:?})", res.data_id, guard.data_map.keys().collect::<Vec<_>>());
                        counter!(
                            "nbx_miner_proto_server_data_id_outdated_or_invalid_count",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).increment(1);
                        counter!("nbx_miner_proto_global_server_data_id_outdated_or_invalid_count").increment(1);
                        continue;
                    };
                    core::mem::drop(guard);

                    counter!(
                        "nbx_miner_proto_server_data_id_valid_count",
                        "client_id" => client_id_str.clone(),
                        "client_name" => client_name.clone(),
                    ).increment(1);
                    counter!(
                        "nbx_miner_proto_cn_server_data_id_valid_count",
                        "client_cn" => client_cn.clone(),
                    ).increment(1);
                    counter!("nbx_miner_proto_global_server_data_id_valid_count").increment(1);

                    histogram!(
                        "nbx_miner_proto_server_attempt_seconds",
                        "client_id" => client_id_str.clone(),
                        "client_name" => client_name.clone(),
                    ).record((res.attempt_millis as f64) / 1000.0);

                    if res.gpu_enqueue_millis > 0 || res.gpu_submit_millis > 0 || res.gpu_wait_millis > 0 {
                        histogram!(
                            "nbx_miner_proto_server_gpu_submit_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).record((res.gpu_submit_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_enqueue_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).record((res.gpu_enqueue_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_wait_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).record((res.gpu_wait_millis as f64) / 1000.0);
                    }

                    let poke = res.poke.map(cue);
                    let effect = res.effect.map(cue);

                    results_out
                        .send(MiningResultOut {
                            miner_metadata: miner.clone(),
                            client_id,
                            data: shared::MiningResult {
                                miner_id: res.miner_id as usize,
                                attempt_millis: res.attempt_millis,
                                gpu_enqueue_millis: res.gpu_enqueue_millis,
                                gpu_submit_millis: res.gpu_submit_millis,
                                gpu_wait_millis: res.gpu_wait_millis,
                                target_hit: res.target_hit,
                                poke,
                                effect,
                            },
                            in_data: data,
                        })
                        .await
                        .map_err(|_| io::ErrorKind::BrokenPipe)?;
                }
            }
        }

        io::Result::Ok(())
    };

    tokio::select! {
        _ = channel_mon => unreachable!(),
        v = sender => v,
        v = receiver => v,
    }
}
