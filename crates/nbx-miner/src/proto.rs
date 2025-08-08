use core::pin::pin;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bincode::{Decode, Encode};
use nockapp::noun::slab::NounSlab;
use rand::random;
use rustls::client::ClientSessionStore;
use strum::FromRepr;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{self};
use tokio::sync::{mpsc, Mutex};
use nbx_jetpack::log::*;
use crate::metrics::{counter, gauge, histogram};

use crate::shared;

pub const PROTOCOL: u32 = 4;

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
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct MiningResult {
    pub data_id: u32,
    pub miner_id: u32,
    pub attempt_millis: u32,
    pub gpu_enqueue_millis: u32,
    pub gpu_submit_millis: u32,
    pub gpu_process_millis: u32,
    pub is_block: bool,
    pub poke: Option<Vec<u8>>,
    pub effect: Option<Vec<u8>>,
}

pub struct MiningResultIn {
    pub data_id: usize,
    pub session_id: u32,
    pub data: shared::MiningResult,
}

pub struct MiningResultOut {
    pub miner_metadata: Arc<BTreeMap<String, Arc<str>>>,
    pub client_id: usize,
    pub data: shared::MiningResult,
}

pub struct MiningDataOut {
    pub server_id: usize,
    pub session_id: u32,
    pub data_id: usize,
    pub data: shared::MiningData,
}

pub struct MiningAckOut {
    pub server_id: usize,
    pub miner_id: usize,
    pub data_id: usize,
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
            let data: MiningData = binrecv(&mut read, server_name.clone(), "mining_data").await?;
            let data_id = data.data_id as usize;
            let data = shared::MiningData {
                block_header: cue(data.block_header),
                version: cue(data.version),
                target: cue(data.target),
                pow_len: data.pow_len,
                block_height: data.block_height,
            };
            gauge!(
                "nbx_miner_proto_client_block_height",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            ).set(data.block_height as f64);
            if mining_data
                .send(MiningDataOut {
                    server_id,
                    data_id,
                    session_id: nonce,
                    data,
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
                data_id: data_id as u32,
                miner_id: data.miner_id as u32,
                attempt_millis: data.attempt_millis,
                gpu_enqueue_millis: data.gpu_enqueue_millis,
                gpu_submit_millis: data.gpu_submit_millis,
                gpu_process_millis: data.gpu_process_millis,
                is_block: data.is_block,
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
    mut mining_data: broadcast::Receiver<Arc<shared::MiningData>>,
    mut cur_mining_data: Option<Arc<shared::MiningData>>,
    client_id: usize,
    results_out: mpsc::Sender<MiningResultOut>,
) -> io::Result<()> {
    let stream = pin!(stream);

    let (mut read, mut write) = split(stream);

    // Initial handshake
    let mut req: Hello = binrecv(&mut read, "".into(), "hello").await?;
    if req.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    req.nonce += 1;

    binsend(
        &mut write,
        "".into(),
        "hello",
        req
    ).await?;

    let client_name: String = binrecv(&mut read, "".into(), "client_name").await?;
    let client_name: Arc<str> = Arc::from(&*client_name);
    let client_id_str: Arc<str> = Arc::from(&*client_id.to_string());

    debug!("Client ID {client_id} joined with name '{client_name}'");

    let data_id = Mutex::new(0);

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
        loop {
            let data = if let Some(data) = cur_mining_data.take() {
                data
            } else {
                match mining_data.recv().await {
                    Ok(data) => data,
                    Err(RecvError::Lagged(m)) => {
                        debug!("Receiver lagged. Skipping {m} messages");
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            };
            // Only send the latest mining data
            if !mining_data.is_empty() {
                continue;
            }
            let mut set_data = MiningData {
                data_id: 0,
                block_header: data.block_header.jam().into(),
                version: data.version.jam().into(),
                target: data.target.jam().into(),
                pow_len: data.pow_len,
                block_height: data.block_height,
            };
            gauge!(
                "nbx_miner_proto_server_block_height",
                "client_id" => client_id_str.clone(),
                "client_name" => client_name.clone(),
            ).set(data.block_height as f64);
            core::mem::drop(data);
            let mut guard = data_id.lock().await;
            *guard += 1;
            set_data.data_id = *guard;
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
        let mut miners = vec![];

        while let Ok(cmd) = read.read_u8().await {
            let Some(cmd) = MinerResponse::from_repr(cmd) else {
                error!("Invalid cmd: {cmd:x}. Exiting");
                return Err(io::ErrorKind::InvalidData.into());
            };
            match cmd {
                MinerResponse::METADATA => {
                    let mdata: SetMinerMetadata = binrecv(&mut read, client_name.clone(), "miner_metadata").await?;
                    miners = mdata.miners.into_iter().map(Arc::new).collect();
                }
                MinerResponse::RESULT => {
                    let res: MiningResult = binrecv(&mut read, client_name.clone(), "mining_result").await?;

                    let Some(miner) = miners.get(res.miner_id as usize) else {
                        counter!(
                            "nbx_miner_proto_server_invalid_miner_id_count",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                        ).increment(1);
                        error!(
                            "Invalid miner id {} (higher than maximum expected)",
                            res.miner_id
                        );
                        return Err(io::ErrorKind::InvalidData.into());
                    };

                    // TODO: optionally include these labels
                    let gpu_index = miner.get("gpu-index").cloned().unwrap_or_default();
                    let gpu_name = miner.get("gpu-name").cloned().unwrap_or_default();

                    let guard = data_id.lock().await;
                    if *guard > res.data_id {
                        counter!(
                            "nbx_miner_proto_server_data_id_outdated_count",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                            "miner_id" => res.miner_id.to_string(),
                            "gpu_index" => gpu_index.clone(),
                            "gpu_name" => gpu_name.clone(),
                        ).increment(1);
                        counter!("nbx_miner_proto_global_server_data_id_outdated_count").increment(1);
                        continue;
                    } else if *guard < res.data_id {
                        counter!(
                            "nbx_miner_proto_server_data_id_invalid_count",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                            "miner_id" => res.miner_id.to_string(),
                            "gpu_index" => gpu_index.clone(),
                            "gpu_name" => gpu_name.clone(),
                        ).increment(1);
                        counter!("nbx_miner_proto_global_server_data_id_invalid_count").increment(1);
                        error!(
                            "Received data_id higher than last sent ({} > {}). Exiting",
                            res.data_id, *guard
                        );
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    core::mem::drop(guard);

                    counter!(
                        "nbx_miner_proto_server_data_id_valid_count",
                        "client_id" => client_id_str.clone(),
                        "client_name" => client_name.clone(),
                        "miner_id" => res.miner_id.to_string(),
                        "gpu_index" => gpu_index.clone(),
                        "gpu_name" => gpu_name.clone(),
                    ).increment(1);
                    counter!("nbx_miner_proto_global_server_data_id_valid_count").increment(1);

                    histogram!(
                        "nbx_miner_proto_server_attempt_seconds",
                        "client_id" => client_id_str.clone(),
                        "client_name" => client_name.clone(),
                        "miner_id" => res.miner_id.to_string(),
                        "gpu_index" => gpu_index.clone(),
                        "gpu_name" => gpu_name.clone(),
                    ).record((res.attempt_millis as f64) / 1000.0);

                    if res.gpu_enqueue_millis > 0 || res.gpu_submit_millis > 0 || res.gpu_process_millis > 0 {
                        histogram!(
                            "nbx_miner_proto_server_gpu_submit_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                            "miner_id" => res.miner_id.to_string(),
                            "gpu_index" => gpu_index.clone(),
                            "gpu_name" => gpu_name.clone(),
                        ).record((res.gpu_submit_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_enqueue_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                            "miner_id" => res.miner_id.to_string(),
                            "gpu_index" => gpu_index.clone(),
                            "gpu_name" => gpu_name.clone(),
                        ).record((res.gpu_enqueue_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_process_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_name" => client_name.clone(),
                            "miner_id" => res.miner_id.to_string(),
                            "gpu_index" => gpu_index.clone(),
                            "gpu_name" => gpu_name.clone(),
                        ).record((res.gpu_process_millis as f64) / 1000.0);
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
                                gpu_process_millis: res.gpu_process_millis,
                                is_block: res.is_block,
                                poke,
                                effect,
                            },
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
