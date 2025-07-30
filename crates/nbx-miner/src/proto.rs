use core::pin::pin;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

use bincode::{Decode, Encode};
use nockapp::noun::slab::NounSlab;
use rand::random;
use strum::FromRepr;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{self};
use tokio::sync::{mpsc, Mutex};
use tracing::*;

use crate::shared;

pub const PROTOCOL: u32 = 1;

#[derive(Encode, Decode, Clone, Debug)]
pub struct Hello {
    protocol: u32,
    nonce: u32,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct SetMinerMetadata {
    miners: Vec<BTreeMap<String, String>>,
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
    pub attempt_seconds: f32,
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
    pub miner_metadata: Arc<BTreeMap<String, String>>,
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

async fn binsend(mut stream: impl AsyncWrite + Unpin, d: impl Encode) -> io::Result<()> {
    let d = bincode::encode_to_vec(d, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;
    stream.write_u32_le(d.len() as _).await?;
    stream.write_all(&d).await?;
    Ok(())
}

async fn binrecv<T: Decode<()>>(mut stream: impl AsyncRead + Unpin) -> io::Result<T> {
    let len = stream.read_u32_le().await?;
    let mut buf = vec![0; len as usize];
    stream.read_exact(&mut buf).await?;
    let (res, _) = bincode::decode_from_slice(&buf, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;
    Ok(res)
}

pub async fn client<S: AsyncRead + AsyncWrite>(
    stream: S,
    server_id: usize,
    mining_out: &mut mpsc::Receiver<MiningResultIn>,
    mining_data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
    metadata: Vec<BTreeMap<String, String>>,
    handshaked: &mut bool,
) -> io::Result<()> {
    let stream = pin!(stream);

    let (mut read, mut write) = split(stream);

    // Initial handshake
    let nonce = random::<u32>();
    binsend(
        &mut write,
        Hello {
            protocol: PROTOCOL,
            nonce,
        },
    )
    .await?;
    let resp: Hello = binrecv(&mut read).await?;
    if resp.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    if resp.nonce != nonce + 1 {
        return Err(io::ErrorKind::BrokenPipe.into());
    }

    *handshaked = true;

    let receiver = async {
        loop {
            let data: MiningData = binrecv(&mut read).await?;
            let data_id = data.data_id as usize;
            let data = shared::MiningData {
                block_header: cue(data.block_header),
                version: cue(data.version),
                target: cue(data.target),
                pow_len: data.pow_len,
                block_height: data.block_height,
            };
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
        binsend(&mut write, SetMinerMetadata { miners: metadata }).await?;

        while let Some(MiningResultIn {
            data_id,
            session_id,
            data,
        }) = mining_out.recv().await
        {
            // Broadcast may contain previous session's datapoints. Skip them.
            if session_id != nonce {
                continue;
            }
            let rdata = MiningResult {
                data_id: data_id as u32,
                miner_id: data.miner_id as u32,
                attempt_seconds: data.attempt_seconds,
                is_block: data.is_block,
                poke: data.poke.as_ref().map(NounSlab::jam).map(Vec::from),
                effect: data.effect.as_ref().map(NounSlab::jam).map(Vec::from),
            };
            write.write_u8(MinerResponse::RESULT as _).await?;
            binsend(&mut write, rdata).await?;
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
    let mut req: Hello = binrecv(&mut read).await?;
    if req.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    req.nonce += 1;
    binsend(&mut write, req).await?;

    let data_id = Mutex::new(0);

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
            core::mem::drop(data);
            let mut guard = data_id.lock().await;
            *guard += 1;
            set_data.data_id = *guard;
            core::mem::drop(guard);
            binsend(&mut write, set_data).await?;
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
                    let mdata: SetMinerMetadata = binrecv(&mut read).await?;
                    miners = mdata.miners.into_iter().map(Arc::new).collect();
                }
                MinerResponse::RESULT => {
                    let res: MiningResult = binrecv(&mut read).await?;

                    let guard = data_id.lock().await;
                    if *guard > res.data_id {
                        continue;
                    } else if *guard < res.data_id {
                        error!(
                            "Received data_id higher than last sent ({} > {}). Exiting",
                            res.data_id, *guard
                        );
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    core::mem::drop(guard);

                    let Some(miner) = miners.get(res.miner_id as usize) else {
                        error!(
                            "Invalid miner id {} (higher than maximum expected)",
                            res.miner_id
                        );
                        return Err(io::ErrorKind::InvalidData.into());
                    };

                    let poke = res.poke.map(cue);
                    let effect = res.effect.map(cue);

                    results_out
                        .send(MiningResultOut {
                            miner_metadata: miner.clone(),
                            client_id,
                            data: shared::MiningResult {
                                miner_id: res.miner_id as usize,
                                attempt_seconds: res.attempt_seconds,
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
        v = sender => v,
        v = receiver => v,
    }
}
