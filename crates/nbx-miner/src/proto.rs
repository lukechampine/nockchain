use core::pin::pin;
use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use bincode::{Decode, Encode};
use chacha::{ChaCha, KeyStream};
use futures::{Stream, StreamExt};
use jsonwebtoken::errors::{Error, ErrorKind};
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use nbx_jetpack::log::*;
use nockapp::noun::slab::NounSlab;
use nockchain_math::belt::Belt;
use rand::random;
use sha3::{Digest, Sha3_256};
use strum::FromRepr;
use tokio::io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::compliance::ip_address_checks::{IpAddressCheckDecision, IpAddressCheckReason};
#[cfg(feature = "compliance")]
use crate::compliance::ipdata::IpAddressChecker;
use crate::device::{Device, DeviceInfoWithSockets};
use crate::metrics::{counter, gauge, histogram};
use crate::shared::{self, JwtClaims};

pub const PROTOCOL: u32 = 7;
pub const NAME_MAX_LENGTH: usize = 16;
pub const DEVICE_ID_LENGTH: usize = 8;
pub const RECENTLY_EXPIRED_DURATION: Duration = Duration::from_secs(20);
pub const PROTO_POW_DIFFICULTY: u32 = 18;
pub const JWT_MAX_LENGTH: usize = 1024;
pub const DEFAULT_MAX_CONNS_FROM_SUB: usize = 10;

// NOTE: This is not proper encryption measure (key is baked into binary). We are just doing this
// to make wireshark analysis much more of a pain to perform. We ensure proper encryption through
// TLS.
pub const CHACHA_KEY: [u8; 32] = [
    0xf9, 0xfb, 0x35, 0x60, 0x88, 0x44, 0xc6, 0xf9, 0xd8, 0xfe, 0x15, 0xe3, 0x22, 0x0e, 0x5b, 0xf5,
    0x01, 0x2a, 0xa0, 0x9f, 0x9e, 0x27, 0xad, 0x0f, 0x6c, 0x20, 0xa5, 0x73, 0xa3, 0xd1, 0xe4, 0x94,
];

pub fn name_valid(name: &str) -> bool {
    name.len() <= NAME_MAX_LENGTH
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn pow_valid(work: u64, nonce: u64, pow_difficulty: u32) -> bool {
    let hash = Sha3_256::digest((((work as u128) << 64) | (nonce as u128)).to_le_bytes());
    let mut leading_zeros = 0;
    for e in hash {
        leading_zeros += e.leading_zeros();
        if e != 0 {
            break;
        }
    }
    leading_zeros >= pow_difficulty
}

fn verify_jwt(jwt: &str, keys: &[DecodingKey]) -> Result<TokenData<JwtClaims>, Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&["nbx-proto"]);
    validation.set_required_spec_claims(&["iss", "aud", "sub"]);
    validation.validate_exp = false;
    for key in keys {
        match decode(jwt, key, &validation).and_then(|v: TokenData<JwtClaims>| {
            if let Some(exp) = v.claims.exp {
                let now = jsonwebtoken::get_current_timestamp();
                if exp < now {
                    return Err(ErrorKind::ExpiredSignature.into());
                } else {
                    Ok(v)
                }
            } else {
                Ok(v)
            }
        }) {
            Err(e) if e.kind() == &ErrorKind::InvalidSignature => continue,
            r => return r,
        }
    }
    Err(ErrorKind::InvalidSignature.into())
}

#[derive(Encode, Decode, Clone, Copy, Debug)]
pub struct Hello {
    protocol: u32,
    client_session_id: u32,
}

#[derive(Encode, Decode, Clone, Copy, Debug)]
pub struct HelloResp {
    protocol: u32,
    nonce_resp: u32,
    pow_nonce: u64,
    pow_difficulty: u32,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct PostHello {
    client_hwid: Arc<str>,
    proof: u64,
    jwt: Option<Arc<str>>,
}

#[derive(Encode, Decode, Clone, Copy, Debug, Default)]
pub struct Permissions {
    pub non_share_proofs: bool,
    pub telemetry_metrics: bool,
    pub telemetry: bool,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct TelemetryHwInfo {
    machines: BTreeMap<Arc<str>, DeviceInfoWithSockets>,
}

#[derive(Encode, Decode, Clone, Debug)]
struct TelemetryProofrate {
    // Proofs per minute
    machines: BTreeMap<Arc<str>, u32>,
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
    pub data: shared::MiningResult,
}

pub enum ClientDataWriteType {
    MiningResult(MiningResultIn),
    Telemetry(shared::Telemetry),
}

pub struct ClientDataWrite {
    pub session_id: u32,
    pub data: ClientDataWriteType,
}

pub enum ClientDataReadType {
    MiningResult(shared::MiningResult, Arc<shared::MiningData>),
    Telemetry(shared::Telemetry),
}

pub struct ClientDataRead {
    pub client_id: usize,
    pub data: ClientDataReadType,
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
#[allow(non_camel_case_types)]
pub enum MinerResponse {
    RESULT = 0,
    TELEMETRY = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[allow(non_camel_case_types)]
pub enum TelemetryResponse {
    HWINFO = 1,
    PROOFRATE = 2,
}

fn cue(d: Vec<u8>) -> NounSlab {
    let mut slab = NounSlab::new();
    let noun = slab.cue_into(d.into()).unwrap();
    slab.set_root(noun);
    slab
}

async fn disconnect<T>(
    stream: impl AsyncWrite + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    d: io::Error,
) -> io::Result<T> {
    let _ = binsend_err(
        stream,
        chacha,
        target_sub,
        target_name.clone(),
        "error".into(),
        &d,
    )
    .await;

    Err(d)
}

async fn binsend_err(
    mut stream: impl AsyncWrite + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
    d: &io::Error,
) -> io::Result<()> {
    let t = Instant::now();
    let mut d = bincode::encode_to_vec(d.to_string(), bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;
    chacha
        .xor_read(&mut d)
        .map_err(|_| io::ErrorKind::BrokenPipe)?;
    stream.write_u32_le((d.len() as u32) | (1u32 << 31)).await?;
    stream.write_all(&d).await?;
    stream.flush().await?;
    #[cfg(not(feature = "production"))]
    histogram!(
        "nbx_miner_binsend_seconds",
        "target_sub" => target_sub.to_string(),
        "target_name" => target_name,
        "msg_type" => msg_type,
    )
    .record(t.elapsed().as_secs_f64());
    Ok(())
}

async fn binsend(
    mut stream: impl AsyncWrite + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
    d: impl Encode,
) -> io::Result<()> {
    let t = Instant::now();
    let mut d = bincode::encode_to_vec(d, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;
    chacha
        .xor_read(&mut d)
        .map_err(|_| io::ErrorKind::BrokenPipe)?;
    stream.write_u32_le(d.len() as _).await?;
    stream.write_all(&d).await?;
    stream.flush().await?;
    #[cfg(not(feature = "production"))]
    histogram!(
        "nbx_miner_binsend_seconds",
        "target_sub" => target_sub.to_string(),
        "target_name" => target_name,
        "msg_type" => msg_type,
    )
    .record(t.elapsed().as_secs_f64());
    Ok(())
}

async fn binrecv<T: Decode<()>, const PARSE_ERR: bool>(
    stream: impl AsyncRead + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
) -> io::Result<T> {
    // 16MB sanity limit
    binrecv_limited::<T, PARSE_ERR, 0x1000000>(stream, chacha, target_sub, target_name, msg_type)
        .await
}

async fn binrecv_server<T: Decode<()>>(
    stream: impl AsyncRead + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
) -> io::Result<T> {
    binrecv::<T, false>(stream, chacha, target_sub, target_name, msg_type).await
}

async fn binrecv_client<T: Decode<()>>(
    stream: impl AsyncRead + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
) -> io::Result<T> {
    binrecv::<T, true>(stream, chacha, target_sub, target_name, msg_type).await
}

async fn binrecv_limited<T: Decode<()>, const PARSE_ERR: bool, const MAX_READ: u32>(
    mut stream: impl AsyncRead + Unpin,
    chacha: &mut ChaCha,
    target_sub: Uuid,
    target_name: Arc<str>,
    msg_type: &'static str,
) -> io::Result<T> {
    let len = stream.read_u32_le().await?;
    let is_err = len & (1u32 << 31) != 0;
    let len = len & !(1u32 << 31);

    let t = Instant::now();

    if len > MAX_READ {
        return Err(io::ErrorKind::OutOfMemory.into());
    }

    if is_err && !PARSE_ERR {
        return Err(io::ErrorKind::Unsupported.into());
    }

    let mut buf = vec![0; len as usize];
    stream.read_exact(&mut buf).await?;
    let _ = chacha
        .xor_read(&mut buf)
        .map_err(|_| io::ErrorKind::BrokenPipe);

    if is_err {
        let (res, _) = bincode::decode_from_slice::<String, _>(&buf, bincode::config::standard())
            .map_err(|_| io::ErrorKind::InvalidData)?;
        return Err(io::Error::new(io::ErrorKind::Interrupted, res));
    }

    let (res, _) = bincode::decode_from_slice(&buf, bincode::config::standard())
        .map_err(|_| io::ErrorKind::InvalidData)?;

    #[cfg(not(feature = "production"))]
    histogram!(
        "nbx_miner_binrecv_seconds",
        "target_sub" => target_sub.to_string(),
        "target_name" => target_name,
        "msg_type" => msg_type,
    )
    .record(t.elapsed().as_secs_f64());

    Ok(res)
}

#[derive(Debug)]
pub struct ClientHandshake<S> {
    pub stream: S,
    pub server_addr: SocketAddr,
    pub server_sub: Uuid,
    pub server_name: Arc<str>,
    pub server_id_str: Arc<str>,
    pub server_id: usize,
    pub session_id: u32,
    pub perms: Permissions,
    pub device: Arc<Device>,
}

#[repr(u8)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum HandshakeStage {
    HelloPing = 0,
    HelloPong,
    PostHelloPing,
    PostHelloPong,
}

pub async fn client_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    server_addr: SocketAddr,
    server_id: usize,
    server_name: &str,
    device: Arc<Device>,
    jwt: Option<Arc<str>>,
    stage: &mut HandshakeStage,
) -> io::Result<ClientHandshake<S>> {
    let (mut read, mut write) = split(&mut stream);

    let server_name: Arc<str> = server_name.into();
    let server_id_str: Arc<str> = Arc::from(&*server_id.to_string());
    let server_sub: Uuid = Uuid::default();

    let mut crypt = ChaCha::new_chacha8(&CHACHA_KEY, &0u64.to_le_bytes());

    // Initial handshake
    *stage = HandshakeStage::HelloPing;
    let session_id = random::<u32>();
    binsend(
        &mut write,
        &mut crypt,
        server_sub.clone(),
        server_name.clone(),
        "hello",
        Hello {
            protocol: PROTOCOL,
            client_session_id: session_id,
        },
    )
    .await?;

    let mut crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());

    *stage = HandshakeStage::HelloPong;
    let resp: HelloResp = binrecv_client(
        &mut read,
        &mut crypt,
        server_sub.clone(),
        server_name.clone(),
        "hello",
    )
    .await?;

    if resp.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }
    if resp.nonce_resp != session_id + 1 {
        return Err(io::ErrorKind::BrokenPipe.into());
    }

    let start = Instant::now();
    let proof = (0..u64::MAX)
        .filter(|i| pow_valid(*i, resp.pow_nonce, resp.pow_difficulty))
        .next()
        .ok_or(io::ErrorKind::InvalidInput)?;
    debug!(
        "Computed PoW in {}ms (proof = {proof})",
        start.elapsed().as_millis()
    );

    *stage = HandshakeStage::PostHelloPing;
    binsend(
        &mut write,
        &mut crypt,
        server_sub.clone(),
        server_name.clone(),
        "post_hello",
        PostHello {
            client_hwid: device.hwid.clone(),
            proof,
            jwt,
        },
    )
    .await?;

    *stage = HandshakeStage::PostHelloPong;
    let perms: Permissions = binrecv_client(
        &mut read,
        &mut crypt,
        server_sub.clone(),
        server_name.clone(),
        "handshake_finish",
    )
    .await?;

    Ok(ClientHandshake {
        stream,
        server_addr,
        server_sub,
        server_name,
        server_id_str,
        server_id,
        session_id,
        perms,
        device,
    })
}

pub async fn client<S: AsyncRead + AsyncWrite + Unpin>(
    ClientHandshake {
        stream,
        server_addr,
        server_sub,
        server_name,
        server_id_str,
        server_id,
        session_id,
        perms,
        device,
    }: ClientHandshake<S>,
    data_out: &mut mpsc::Receiver<ClientDataWrite>,
    mining_data: mpsc::Sender<MiningDataOut>,
    ack: mpsc::Sender<MiningAckOut>,
) -> io::Result<()> {
    let (mut read, mut write) = split(stream);

    let mut read_crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());
    let mut write_crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());

    #[cfg(feature = "production")]
    let channel_mon = std::future::pending::<()>();

    #[cfg(not(feature = "production"))]
    let channel_mon = async {
        loop {
            gauge!(
                "nbx_miner_proto_client_channel_capacity",
                "channel_name" => "mining_data",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            )
            .set(mining_data.capacity() as f64);
            gauge!(
                "nbx_miner_proto_client_channel_capacity",
                "channel_name" => "ack",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            )
            .set(ack.capacity() as f64);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };

    let receiver = async {
        loop {
            let datas: MiningDatas = binrecv_client(
                &mut read,
                &mut read_crypt,
                server_sub.clone(),
                server_name.clone(),
                "mining_data",
            )
            .await?;
            gauge!(
                "nbx_miner_proto_client_block_height",
                "server_name" => server_name.clone(),
                "server_id" => server_id_str.clone(),
            )
            .set(
                datas
                    .new_datas
                    .iter()
                    .map(|v| v.block_height)
                    .max()
                    .unwrap_or_default() as f64,
            );
            if mining_data
                .send(MiningDataOut {
                    server_id,
                    session_id,
                    expire: datas.expire,
                    new_datas: datas
                        .new_datas
                        .into_iter()
                        .map(|data| {
                            (
                                data.data_id,
                                shared::MiningData {
                                    block_header: cue(data.block_header),
                                    version: cue(data.version),
                                    target: cue(data.target),
                                    pow_len: data.pow_len,
                                    block_height: data.block_height,
                                    fixed_nonce_atoms: data
                                        .fixed_nonce_atoms
                                        .into_iter()
                                        .map(Belt)
                                        .collect(),
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                })
                .await
                .is_err()
            {
                break io::Result::Ok(());
            }
        }
    };

    let sender = async {
        // TODO: inject this inside the loop
        if perms.telemetry {
            write.write_u8(MinerResponse::TELEMETRY as _).await?;
            write.write_u8(TelemetryResponse::HWINFO as _).await?;
            binsend(
                &mut write,
                &mut write_crypt,
                server_sub.clone(),
                server_name.clone(),
                "telemetry_hwinfo",
                TelemetryHwInfo {
                    machines: [(
                        device.hwid.clone(),
                        device.info.with_outgoing_socket(server_addr),
                    )]
                    .into_iter()
                    .collect(),
                },
            )
            .await?;
        }

        while let Some(ClientDataWrite {
            session_id: data_session_id,
            data,
        }) = data_out.recv().await
        {
            // Broadcast may contain previous session's datapoints. Skip them.
            if data_session_id != session_id {
                #[cfg(not(feature = "production"))]
                counter!(
                    "nbx_miner_proto_client_session_id_mismatch_count",
                    "server_name" => server_name.clone(),
                    "server_id" => server_id_str.clone(),
                )
                .increment(1);
                continue;
            }
            match data {
                ClientDataWriteType::MiningResult(MiningResultIn { data_id, data }) => {
                    let miner_id: Arc<str> = Arc::from(&*data.miner_id.to_string());
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
                    )
                    .set(rdata.data_id as f64);
                    write.write_u8(MinerResponse::RESULT as _).await?;
                    binsend(
                        &mut write,
                        &mut write_crypt,
                        server_sub.clone(),
                        server_name.clone(),
                        "mining_result",
                        rdata,
                    )
                    .await?;
                    let _ = ack
                        .send(MiningAckOut {
                            server_id,
                            miner_id: data.miner_id,
                            data_id,
                        })
                        .await;
                }
                ClientDataWriteType::Telemetry(telemetry) => {
                    write.write_u8(MinerResponse::TELEMETRY as _).await?;
                    match telemetry {
                        shared::Telemetry::Proofrate { machines } => {
                            let telemetry = TelemetryProofrate { machines };
                            write.write_u8(TelemetryResponse::PROOFRATE as _).await?;
                            binsend(
                                &mut write,
                                &mut write_crypt,
                                server_sub.clone(),
                                server_name.clone(),
                                "telemetry_proofrate",
                                telemetry,
                            )
                            .await?;
                        }
                        shared::Telemetry::HwInfo { mut machines } => {
                            machines
                                .values_mut()
                                .for_each(|v| v.sockets_outgoing.push(server_addr));
                            let telemetry = TelemetryHwInfo { machines };
                            write.write_u8(TelemetryResponse::HWINFO as _).await?;
                            binsend(
                                &mut write,
                                &mut write_crypt,
                                server_sub.clone(),
                                server_name.clone(),
                                "telemetry_hwinfo",
                                telemetry,
                            )
                            .await?;
                        }
                    }
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

#[derive(Debug)]
pub struct ServerHandshake<S> {
    pub stream: S,
    pub client_addr: SocketAddr,
    pub client_sub: Uuid,
    pub client_hwid: Arc<str>,
    pub perms: Permissions,
    pub session_id: u32,
    pub conn: shared::ConnHandle,
}

pub async fn server_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    client_addr: SocketAddr,
    jwt_keys: Arc<[DecodingKey]>,
    conntrack: shared::ConnTrack,
    #[cfg(feature = "db")] db: Option<crate::db::DatabaseHandle>,
    #[cfg(feature = "compliance")] ip_checker: Option<IpAddressChecker>,
) -> io::Result<ServerHandshake<S>> {
    let mut crypt = ChaCha::new_chacha8(&CHACHA_KEY, &0u64.to_le_bytes());

    // Initial handshake. We only have 8 bytes in the hello packet, but then there's extra padding
    let req: Hello = binrecv_limited::<_, false, 16>(
        &mut stream,
        &mut crypt,
        Uuid::default(),
        "".into(),
        "hello",
    )
    .await?;
    if req.protocol != PROTOCOL {
        return Err(io::ErrorKind::Unsupported.into());
    }

    let session_id = req.client_session_id;
    let mut crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());

    let resp = HelloResp {
        protocol: PROTOCOL,
        nonce_resp: req.client_session_id + 1,
        pow_nonce: random::<u64>(),
        pow_difficulty: PROTO_POW_DIFFICULTY,
    };

    binsend(
        &mut stream,
        &mut crypt,
        Uuid::default(),
        "".into(),
        "hello",
        resp,
    )
    .await?;

    let PostHello {
        proof,
        client_hwid,
        jwt,
    } = binrecv_limited::<_, false, { NAME_MAX_LENGTH as u32 + JWT_MAX_LENGTH as u32 + 16 }>(
        &mut stream,
        &mut crypt,
        Uuid::default(),
        "".into(),
        "post_hello",
    )
    .await?;

    if !pow_valid(proof, resp.pow_nonce, resp.pow_difficulty) {
        return Err(io::ErrorKind::InvalidData.into());
    }

    if !name_valid(&client_hwid) {
        return Err(io::ErrorKind::InvalidData.into());
    }

    let claims = match (jwt, jwt_keys) {
        (a, b) if a.is_none() && b.is_empty() => {
            trace!("JWT validation skipped");
            JwtClaims {
                sub: Default::default(),
                iat: None,
                exp: None,
                non_share_proofs: true,
                telemetry: true,
                telemetry_metrics: true,
                max_conns_override: Some(usize::MAX),
            }
        }
        (jwt, jwt_keys) => {
            let jwt = jwt.unwrap_or_default();
            let token = match verify_jwt(&jwt, &jwt_keys) {
                Ok(t) => t,
                Err(e) => {
                    let _ = binsend_err(
                        stream,
                        &mut crypt,
                        Uuid::default(),
                        "".into(),
                        "error".into(),
                        &io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!("invalid JWT: {e}"),
                        ),
                    )
                    .await;
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
            };
            trace!("JWT valid: {:?}", token.claims);
            token.claims
        }
    };

    let client_sub: Uuid = claims.sub;
    let client_hwid: Arc<str> = (*client_hwid).into();

    #[cfg(feature = "compliance")]
    {
        if let Some(db) = &db {
            let (is_blocklisted, blocklisted_message) = db.is_blocklisted(client_sub).await;

            if is_blocklisted {
                counter!("nbx_miner_proto_server_blocklist_rejection_total").increment(1);
                warn!("Rejecting blocklisted user: sub={client_sub}, hwid={client_hwid}");

                return disconnect(
                    stream,
                    &mut crypt,
                    client_sub,
                    client_hwid.clone(),
                    io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        blocklisted_message
                            .unwrap_or_else(|| "Nockbox is not available for you.".to_string()),
                    ),
                )
                .await;
            }

            let client_ip = client_addr.ip();

            if let Some(ip_checker) = ip_checker {
                let block_message = match ip_checker.check_ip(client_ip, client_sub, db).await
                {
                    None => Some("NockBox is unavailable for you"),
                    Some(IpAddressCheckDecision::Allow) => None,
                    Some(IpAddressCheckDecision::Block(reason))
                    | Some(IpAddressCheckDecision::Review(reason)) => {
                        Some(match reason {
                            IpAddressCheckReason::Country => "NockBox is unavailable in your location.",
                            IpAddressCheckReason::Vpn => "Please disable IP anonymizer or complete identity verification, reach out to support@nockbox.org”",
                        })
                    }
                };

                if let Some(block_message) = block_message {
                    if db.is_allow_listed(client_sub).await {
                        info!("Sub {client_sub} is allowlisted, ignoring block check decision for {client_ip}");
                        counter!("nbx_miner_proto_allowlist_bypass_total").increment(1);
                    } else {
                        counter!("nbx_miner_proto_ip_block_rejection_total", "client_sub" => client_sub.to_string()).increment(1);

                        db.blocklist(client_sub, block_message.to_string());

                        return disconnect(
                            stream,
                            &mut crypt,
                            client_sub,
                            client_hwid.clone(),
                            io::Error::new(io::ErrorKind::ConnectionRefused, block_message),
                        )
                        .await;
                    }
                }
            }
        }
    }

    let Some(conn) = conntrack.connect(
        client_sub,
        client_hwid.clone(),
        claims
            .max_conns_override
            .unwrap_or(DEFAULT_MAX_CONNS_FROM_SUB),
    ) else {
        return disconnect(
            stream,
            &mut crypt,
            client_sub,
            client_hwid.clone(),
            io::Error::new(io::ErrorKind::ConnectionRefused, "Too many connections"),
        )
        .await;
    };

    let perms = Permissions {
        non_share_proofs: claims.non_share_proofs,
        telemetry: claims.telemetry,
        telemetry_metrics: claims.telemetry_metrics,
    };

    binsend(
        &mut stream,
        &mut crypt,
        client_sub.clone(),
        client_hwid.clone(),
        "handshake_finish",
        perms,
    )
    .await?;

    debug!("Client with subject '{client_sub}' and HWID '{client_hwid}' completed handshake");

    Ok(ServerHandshake {
        stream,
        client_addr,
        client_sub,
        client_hwid,
        perms,
        session_id,
        conn,
    })
}

pub async fn server<S: AsyncRead + AsyncWrite + Unpin>(
    handshake: ServerHandshake<S>,
    mining_data: impl Stream<Item = (Arc<shared::MiningData>, Arc<OnceLock<Instant>>)>,
    client_id: usize,
    results_out: mpsc::Sender<ClientDataRead>,
    graceful_stop: oneshot::Receiver<()>,
) -> io::Result<()> {
    let ServerHandshake {
        stream,
        client_addr,
        client_sub,
        client_hwid,
        perms,
        session_id,
        conn: _conn,
    } = handshake;
    debug!("Client ID {client_id} joined with subject '{client_sub}' and HWID '{client_hwid}'");

    let mut mining_data = pin!(mining_data);

    let (mut read, mut write) = split(stream);
    let mut read_crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());
    let mut write_crypt = ChaCha::new_chacha8(&CHACHA_KEY, &(session_id as u64).to_le_bytes());

    let client_id_str: Arc<str> = Arc::from(&*client_id.to_string());

    #[derive(Default)]
    struct DataTracker {
        data_id: u32,
        data_map: BTreeMap<u32, (Arc<shared::MiningData>, Arc<OnceLock<Instant>>, bool)>,
        recently_expired_map: BTreeMap<u32, (Arc<shared::MiningData>, Instant)>,
    }

    impl DataTracker {
        fn add_data(
            &mut self,
            data: Arc<shared::MiningData>,
            expire: Arc<OnceLock<Instant>>,
        ) -> u32 {
            let data_id = self.data_id;
            self.data_id = self.data_id.wrapping_add(1);
            self.data_map.insert(data_id, (data, expire, false));
            data_id
        }

        fn expire_all(&mut self) {
            self.data_map
                .values_mut()
                .for_each(|(_, _, force_expire)| *force_expire = true);
        }

        fn remove_recently_expired(&mut self) {
            // Retain only for last 10 seconds, as to give enough time for shares to be submitted.
            self.recently_expired_map
                .retain(|_, v| v.1.elapsed() < RECENTLY_EXPIRED_DURATION);
        }

        fn collect_expired(&mut self) -> Vec<u32> {
            self.remove_recently_expired();
            let expired = self
                .data_map
                .iter()
                .filter(|(_, (_, v, e))| v.get().is_some() || *e)
                .map(|(v, _)| *v)
                .collect::<Vec<_>>();
            for i in &expired {
                let d = self.data_map.remove(&i).unwrap();
                self.recently_expired_map
                    .insert(*i, (d.0, d.1.get().copied().unwrap_or_else(Instant::now)));
            }
            expired
        }

        fn valid_data(&mut self, data_id: u32) -> Option<Arc<shared::MiningData>> {
            self.remove_recently_expired();
            self.data_map
                .get(&data_id)
                .and_then(|(v, e, _)| {
                    if e.get()
                        .filter(|v| v.elapsed() >= RECENTLY_EXPIRED_DURATION)
                        .is_none()
                    {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    self.recently_expired_map
                        .get(&data_id)
                        .map(|(v, _)| v.clone())
                })
        }
    }

    let tracker = Mutex::new(DataTracker::default());

    #[cfg(feature = "production")]
    let channel_mon = std::future::pending::<()>();

    #[cfg(not(feature = "production"))]
    let channel_mon = async {
        loop {
            gauge!(
                "nbx_miner_proto_server_channel_capacity",
                "channel_name" => "results_out",
                "client_sub" => client_sub.to_string(),
                "client_hwid" => client_hwid.clone(),
                "client_id" => client_id_str.clone(),
            )
            .set(results_out.capacity() as f64);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    let sender = async {
        let mut expiries = JoinSet::new();
        let mut graceful_stop = pin!(graceful_stop);

        loop {
            let graceful_stop = graceful_stop.as_mut();
            let is_stopping = graceful_stop.is_terminated();

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

                    if expiration.get().is_none() && !is_stopping {
                        gauge!(
                            "nbx_miner_proto_server_block_height",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
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
                v = async {
                    if is_stopping {
                        tokio::time::sleep(RECENTLY_EXPIRED_DURATION).await;
                        None
                    } else {
                        Some(graceful_stop.await)
                    }
                } => {
                    let Some(Ok(_)) = v else {
                        debug!("Aborting the send loop");
                        break;
                    };
                    debug!("Gracefully disconnecting client_id={client_id}, client_sub={client_sub}, client_hwid={client_hwid}");
                    counter!(
                        "nbx_proto_server_graceful_stop_count",
                        "client_id" => client_id_str.clone(),
                        "client_sub" => client_sub.to_string(),
                        "client_hwid" => client_hwid.clone(),
                    ).increment(1);
                    counter!(
                        "nbx_proto_server_global_graceful_stop_count",
                    ).increment(1);
                    let mut guard = tracker.lock().await;
                    guard.expire_all();
                    guard
                }
            };

            set_data.expire = guard.collect_expired();
            core::mem::drop(guard);

            binsend(
                &mut write,
                &mut write_crypt,
                client_sub.clone(),
                client_hwid.clone(),
                "mining_data",
                set_data,
            )
            .await?;
            counter!(
                "nbx_miner_proto_server_set_data_count",
                "client_id" => client_id_str.clone(),
                "client_sub" => client_sub.to_string(),
                "client_hwid" => client_hwid.clone(),
            )
            .increment(1);
        }

        io::Result::Ok(())
    };

    let receiver = async {
        while let Ok(cmd) = read.read_u8().await {
            let Some(cmd) = MinerResponse::from_repr(cmd) else {
                error!("Invalid cmd: {cmd:x}. Exiting");
                return Err(io::ErrorKind::InvalidData.into());
            };
            match cmd {
                MinerResponse::RESULT => {
                    let res: MiningResult = binrecv_server(
                        &mut read,
                        &mut read_crypt,
                        client_sub.clone(),
                        client_hwid.clone(),
                        "mining_result",
                    )
                    .await?;

                    if !res.target_hit && !perms.non_share_proofs {
                        counter!(
                            "nbx_miner_proto_server_unauthenticated_non_share_proof_count",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .increment(1);
                        trace!("client_id={client_id}, client_sub={client_sub}, client_hwid={client_hwid} sent non-share proof without the capability. Ignoring.");
                        continue;
                    }

                    let mut guard = tracker.lock().await;
                    let Some(data) = guard.valid_data(res.data_id) else {
                        trace!(
                            "Unable to find data with ID {} ({:?})",
                            res.data_id,
                            guard.data_map.keys().collect::<Vec<_>>()
                        );
                        counter!(
                            "nbx_miner_proto_server_data_id_outdated_or_invalid_count",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .increment(1);
                        counter!("nbx_miner_proto_global_server_data_id_outdated_or_invalid_count")
                            .increment(1);
                        continue;
                    };
                    core::mem::drop(guard);

                    counter!(
                        "nbx_miner_proto_server_data_id_valid_count",
                        "client_id" => client_id_str.clone(),
                        "client_sub" => client_sub.to_string(),
                        "client_hwid" => client_hwid.clone(),
                    )
                    .increment(1);
                    counter!(
                        "nbx_miner_proto_sub_server_data_id_valid_count",
                        "client_sub" => client_sub.to_string(),
                    )
                    .increment(1);
                    counter!("nbx_miner_proto_global_server_data_id_valid_count").increment(1);

                    histogram!(
                        "nbx_miner_proto_server_attempt_seconds",
                        "client_id" => client_id_str.clone(),
                        "client_sub" => client_sub.to_string(),
                        "client_hwid" => client_hwid.clone(),
                    )
                    .record((res.attempt_millis as f64) / 1000.0);

                    // FIXME: let's not expose this until we figure out better GPU readings and
                    // metrics isolation from public binaries.
                    /*if res.gpu_enqueue_millis > 0
                        || res.gpu_submit_millis > 0
                        || res.gpu_wait_millis > 0
                    {
                        histogram!(
                            "nbx_miner_proto_server_gpu_submit_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .record((res.gpu_submit_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_enqueue_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .record((res.gpu_enqueue_millis as f64) / 1000.0);

                        histogram!(
                            "nbx_miner_proto_server_gpu_wait_seconds",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .record((res.gpu_wait_millis as f64) / 1000.0);
                    }*/

                    let poke = res.poke.map(cue);
                    let effect = res.effect.map(cue);

                    results_out
                        .send(ClientDataRead {
                            client_id,
                            data: ClientDataReadType::MiningResult(
                                shared::MiningResult {
                                    miner_id: res.miner_id as usize,
                                    attempt_millis: res.attempt_millis,
                                    gpu_enqueue_millis: res.gpu_enqueue_millis,
                                    gpu_submit_millis: res.gpu_submit_millis,
                                    gpu_wait_millis: res.gpu_wait_millis,
                                    target_hit: res.target_hit,
                                    poke,
                                    effect,
                                },
                                data,
                            ),
                        })
                        .await
                        .map_err(|_| io::ErrorKind::BrokenPipe)?;
                }
                MinerResponse::TELEMETRY => {
                    if !perms.telemetry {
                        counter!(
                            "nbx_miner_proto_server_unauthenticated_telemetry_count",
                            "client_id" => client_id_str.clone(),
                            "client_sub" => client_sub.to_string(),
                            "client_hwid" => client_hwid.clone(),
                        )
                        .increment(1);
                        trace!("client_id={client_id}, client_sub={client_sub}, client_hwid={client_hwid} sent telemetry without being allowed to do it. Aborting.");
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "Telemetry not allowed",
                        ));
                    }

                    let cmd = read.read_u8().await?;
                    let Some(cmd) = TelemetryResponse::from_repr(cmd) else {
                        error!("Invalid telemetry cmd: {cmd:x}. Exiting");
                        return Err(io::ErrorKind::InvalidData.into());
                    };

                    let data = match cmd {
                        TelemetryResponse::PROOFRATE => {
                            let TelemetryProofrate { machines } = binrecv_server(
                                &mut read,
                                &mut read_crypt,
                                client_sub.clone(),
                                client_hwid.clone(),
                                "telemetry_proofrate",
                            )
                            .await?;
                            if machines.keys().any(|v| v.len() > DEVICE_ID_LENGTH) {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "Proofrate includes machine IDs beyond allowed length",
                                ));
                            }
                            ClientDataReadType::Telemetry(shared::Telemetry::Proofrate { machines })
                        }
                        TelemetryResponse::HWINFO => {
                            let TelemetryHwInfo { mut machines } = binrecv_server(
                                &mut read,
                                &mut read_crypt,
                                client_sub.clone(),
                                client_hwid.clone(),
                                "telemetry_hwinfo",
                            )
                            .await?;
                            if machines.keys().any(|v| v.len() > DEVICE_ID_LENGTH) {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "HwInfo includes machine IDs beyond allowed length",
                                ));
                            }
                            machines
                                .values_mut()
                                .for_each(|v| v.sockets_incoming.push(client_addr));
                            ClientDataReadType::Telemetry(shared::Telemetry::HwInfo { machines })
                        }
                    };

                    results_out
                        .send(ClientDataRead { client_id, data })
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
