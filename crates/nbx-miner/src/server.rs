use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::future::Future;
use std::io;
use std::net::{Ipv6Addr, SocketAddr};
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
use clap_serde_derive::ClapSerde;
use futures::stream::StreamExt;
use jsonwebtoken::DecodingKey;
#[cfg(feature = "verifier")]
use kernels::verifier::KERNEL;
use metrics::{counter, gauge};
use nbx_jetpack::log::*;
use nockapp::driver::NockAppHandle;
use nockapp::noun::slab::NounSlab;
use nockapp::wire::Wire;
#[cfg(feature = "verifier")]
use nockapp::{
    kernel::form::SerfThread, noun::slab::NockJammer, save::SaveableCheckpoint,
    utils::NOCK_STACK_SIZE_TINY,
};
use nockapp::{NockAppError, NounExt};
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockchain_math::belt::*;
use nockchain_math::noun_ext::NounMathExt;
use nockvm::noun::D;
#[cfg(feature = "verifier")]
use nockvm::noun::T;
#[cfg(feature = "verifier")]
use nockvm_macros::tas;
use rand::seq::SliceRandom;
use rustls::crypto::ring::default_provider;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
#[cfg(feature = "verifier")]
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::{AbortHandle, Id, JoinSet};
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[cfg(feature = "compliance")]
use crate::compliance::{blocklist::spawn_blocklist_refresh_task, ipdata::IpAddressChecker};
#[cfg(feature = "db")]
use crate::difficulty_buckets::spawn_difficulty_refresh_task;
use crate::proto::{server, server_handshake, ClientDataRead, ClientDataReadType};
use crate::shared::{
    tls_accept, ConnTrack, MiningData, MiningResult, MiningWire, Telemetry, TimeWriter,
    TlsServerConfig,
};

pub const TELEMETRY_PROOFRATE_INTERVAL: Duration = Duration::from_secs(60);
const LOG_TARGET: &str = "nbx::server";

#[derive(ClapSerde, Args, Clone, Debug, Serialize, Deserialize)]
pub struct MiningConfig {
    #[default(None)]
    #[arg(long, help = "Where to bind the mining server to [default: [::]:4344]")]
    pub miner_bind: Option<SocketAddr>,
    #[cfg(feature = "server-tls-key-load")]
    #[arg(long, help = "Path to custom TLS private key")]
    miner_tls_key: Option<String>,
    #[cfg(feature = "server-tls-key-load")]
    #[arg(long, help = "Path to custom TLS certificate chain")]
    miner_tls_chain: Option<String>,
    #[cfg(all(feature = "verifier", not(feature = "force-preverify")))]
    #[arg(
        long,
        help = "Whether to pre-verify client proofs before accepting them as valid"
    )]
    miner_preverify: bool,
    #[default(None)]
    #[cfg(feature = "miner-save-attempts")]
    #[arg(long, help = "Which mining attempts to save [default: none]")]
    miner_save_attempts: Option<SaveMineAttempts>,
    #[cfg(feature = "jwt-auth-server")]
    #[arg(
        long = "miner-jwt-key",
        help = "JWT keys to verify client connections with. Multiple to allow failover. Used in addition to NBX_JWT_KEY[1-9] environment variables."
    )]
    miner_jwt_keys: Vec<String>,
    #[default(None)]
    #[arg(
        long,
        help = "Fraction of connections to drop on an interval. Helps equalize connections."
    )]
    miner_conndrop_fraction: Option<f64>,
    #[default(None)]
    #[arg(long, help = "Seconds interval to activate conndrop. [default: 60]")]
    miner_conndrop_interval_seconds: Option<u64>,
}

impl MiningConfig {
    pub fn miner_bind(&self) -> SocketAddr {
        self.miner_bind
            .unwrap_or_else(|| (Ipv6Addr::LOCALHOST, 4344).into())
    }
}

type Result<T = ()> = core::result::Result<T, NockAppError>;

pub async fn bind(cfg: &MiningConfig) -> Result<TcpListener> {
    let _ = default_provider().install_default();

    let listener = TcpListener::bind(cfg.miner_bind())
        .await
        .map_err(NockAppError::IoError)?;
    info!(
        "Bound on {}",
        listener.local_addr().map_err(NockAppError::IoError)?
    );
    Ok(listener)
}

#[derive(Default, Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Serialize, Deserialize)]
enum SaveMineAttempts {
    #[default]
    None,
    All,
    TargetHit,
}

impl SaveMineAttempts {
    fn should_save_lucky(self) -> bool {
        matches!(self, Self::All | Self::TargetHit)
    }

    fn should_save_unlucky(self) -> bool {
        matches!(self, Self::All)
    }
}

async fn save_mine_attempt(inp: &NounSlab, res: &NounSlab, run_id: &str, run_cnt: usize) {
    let in_jam = inp.jam();
    let out_jam = res.jam();
    let dir = std::path::Path::new("miner_jams")
        .join(&run_id)
        .join(run_cnt.to_string());
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let _ = tokio::fs::write(dir.join("event.jam"), in_jam).await;
    let _ = tokio::fs::write(dir.join("effect.jam"), out_jam).await;
}

#[derive(Clone)]
pub(crate) enum AbortReason {
    NounValidation(&'static str),
    ValidatorFailure(&'static str),
    ProofValidation(String),
    ProofRejected,
    TelemetryError,
    Blocklisted,
}

impl ToString for AbortReason {
    fn to_string(&self) -> String {
        match self {
            Self::NounValidation(v) => format!("abort::noun_validation: {v}"),
            Self::ValidatorFailure(v) => format!("abort::validation_failure: {v}"),
            Self::ProofValidation(v) => {
                format!("abort::proof_validation: validator rejected the proof, why={v}")
            }
            Self::ProofRejected => format!("abort::proof_rejected: Mined PoW was not accepted"),
            Self::TelemetryError => format!("abort::telemetry_error: Telemetry error"),
            Self::Blocklisted => format!("abort::blocklisted: User has been blocklisted"),
        }
    }
}

impl AbortReason {
    pub fn log(self, client_id: usize) {
        error!("{}. client_id={client_id}", self.to_string());
    }

    #[rustfmt::skip]
    pub fn emit_metrics(&self) {
        match self {
            Self::NounValidation(_) => counter!("nbx_miner_server_abort_count", "mode" => "noun_validation").increment(1),
            Self::ValidatorFailure(_) => counter!("nbx_miner_server_abort_count", "mode" => "validator_failure").increment(1),
            Self::ProofValidation(_) => counter!("nbx_miner_server_abort_count", "mode" => "proof_validation").increment(1),
            Self::ProofRejected => counter!("nbx_miner_server_abort_count", "mode" => "proof_rejected").increment(1),
            Self::TelemetryError => counter!("nbx_miner_server_abort_count", "mode" => "telemetry_error").increment(1),
            Self::Blocklisted => counter!("nbx_miner_server_abort_count", "mode" => "blocklisted").increment(1),
        }
    }
}

pub(crate) struct Clients<M> {
    handles: BTreeMap<usize, (AbortHandle, Option<oneshot::Sender<()>>, M)>,
    ids: HashMap<Id, usize>,
    subs: HashMap<Uuid, BTreeSet<usize>>,
}

impl<M> Default for Clients<M> {
    fn default() -> Self {
        Self {
            handles: Default::default(),
            ids: Default::default(),
            subs: Default::default(),
        }
    }
}

impl<M> Clients<M> {
    pub fn abort(&mut self, client: usize, reason: AbortReason) {
        debug!("Aborting client_id={client}");
        reason.emit_metrics();
        reason.log(client);
        self.handles.get(&client).unwrap().0.abort();
    }

    pub fn abort_by_sub(&mut self, sub: &Uuid, reason: AbortReason) -> usize {
        let Some(client_ids) = self.subs.get(sub) else {
            return 0;
        };

        let client_ids: Vec<usize> = client_ids.iter().copied().collect();
        let count = client_ids.len();

        for client_id in client_ids {
            self.abort(client_id, reason.clone());
        }

        count
    }

    pub fn remove(&mut self, id: Id) -> Option<usize> {
        let client = self.ids.remove(&id)?;
        self.handles.remove(&client);
        self.subs.retain(|_, clients| {
            clients.remove(&client);
            !clients.is_empty()
        });
        Some(client)
    }

    pub fn add(
        &mut self,
        client: usize,
        handle: AbortHandle,
        graceful_stop: oneshot::Sender<()>,
        metadata: M,
        sub: Uuid,
    ) {
        self.ids.insert(handle.id(), client);
        self.handles
            .insert(client, (handle, Some(graceful_stop), metadata));
        self.subs.entry(sub).or_default().insert(client);
    }

    pub fn lookup(&self, client: usize) -> Option<&M> {
        self.handles.get(&client).map(|(_, _, v)| v)
    }

    pub fn drop_fraction(&mut self, frac: f64) {
        let mut handles = self.handles.values_mut().collect::<Vec<_>>();
        let amt = core::cmp::min(
            ((handles.len() as f64) * frac.min(1.0).max(0.0)) as usize,
            handles.len(),
        );
        let handles = handles.partial_shuffle(&mut rand::thread_rng(), amt).0;
        counter!("nbx_miner_server_dropped_count").increment(handles.len() as u64);
        for h in handles {
            if let Some(h) = h.1.take() {
                let _ = h.send(());
            } else {
                debug!("No graceful stop handle. Aborting.");
                h.0.abort();
            }
        }
    }
}

pub async fn mining_driver(
    cfg: MiningConfig,
    listener: TcpListener,
    handle: NockAppHandle,
) -> Result {
    let (reqs_out, reqs_in) = mpsc::channel(1);

    let process_target = |data: MiningResult,
                          client_id: usize,
                          sub: Uuid,
                          hwid: Arc<str>,
                          _,
                          poke_slab: NounSlab| {
        info!(
            "Found block! client={} hwid={hwid} subject={sub} miner={}",
            client_id, data.miner_id
        );
        let fut = handle.poke(MiningWire::Mined.to_wire(), poke_slab);
        async move { fut.await.map(|_| ()) }
    };

    let process_telemetry =
        |_telemetry: Telemetry, _client_id: usize, _sub: Uuid| async move { Result::Ok(()) };

    let server = mining_server(
        cfg,
        listener,
        [reqs_in],
        process_target,
        process_telemetry,
        None,
        #[cfg(feature = "db")]
        None,
        #[cfg(feature = "compliance")]
        None,
    );
    let mut server = pin!(server);
    let mut cur_mining_data: Option<(Arc<MiningData>, _)> = None;
    let mut randbelt = Belt(0);

    loop {
        tokio::select! {
            _ = &mut server => break,
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
                    let (version_slab, header_slab, target_slab, pow_len, block_height) = {
                        let [version, commit, target, pow_len_noun, height_noun] = effect_cell.tail().uncell().expect(
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
                        let block_height =
                            height_noun
                                .as_atom()
                                .expect("Expected block-height to be an atom")
                                .as_u64()
                                .expect("Expected block-height to be a u64");
                        (version_slab, header_slab, target_slab, pow_len, block_height)
                    };
                    debug!("received new candidate block header: {:?}",
                        tip5_hash_to_base58(*unsafe { header_slab.root() })
                        .expect("Failed to convert header to Base58")
                    );

                    let mut new_mining_data = MiningData {
                        block_header: header_slab,
                        version: version_slab,
                        target: target_slab,
                        pow_len,
                        block_height,
                        fixed_nonce_atoms: vec![randbelt],
                    };

                    if cur_mining_data.as_ref().map(|(v, _)| &**v) != Some(&new_mining_data) {
                        randbelt = Belt(rand::random::<u64>() % PRIME);
                        new_mining_data.fixed_nonce_atoms[0] = randbelt;
                        let new_mining_data = Arc::new(new_mining_data);
                        let (inst_handle, inst) = TimeWriter::new();
                        cur_mining_data = Some((new_mining_data.clone(), inst_handle));
                        debug!("Creating new mining attempts");
                        if reqs_out.send((new_mining_data, inst)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn mining_server<
    F1: FnMut(MiningResult, usize, Uuid, Arc<str>, Arc<MiningData>, NounSlab) -> Fut1,
    F2: FnMut(Telemetry, usize, Uuid) -> Fut2,
    Fut1: Future<Output = Result>,
    Fut2: Future<Output = Result>,
>(
    cfg: MiningConfig,
    listener: TcpListener,
    mut reqs_in: impl AsMut<[mpsc::Receiver<(Arc<MiningData>, Arc<OnceLock<Instant>>)>]>,
    mut process_target: F1,
    mut process_telemetry: F2,
    connected_clients: Option<&AtomicUsize>,
    #[cfg(feature = "db")] db: Option<crate::db::DatabaseHandle>,
    #[cfg(feature = "compliance")] ip_checker: Option<IpAddressChecker>,
) -> Result {
    let reqs_in = reqs_in.as_mut();
    let (tx, mut rx) = mpsc::channel(1024);
    let mining_data_tx = (0..reqs_in.len())
        .map(|_| broadcast::channel(16).0)
        .collect::<Vec<_>>();
    let mut replay_mining_data =
        vec![VecDeque::<(Arc<MiningData>, Arc<OnceLock<Instant>>)>::new(); reqs_in.len()];

    let mut client_set = JoinSet::new();
    let mut clients = Clients::default();
    let mut client_cnt = 0;

    #[cfg(feature = "server-tls-key-load")]
    let tls = match (cfg.miner_tls_key, cfg.miner_tls_chain) {
        (Some(key), Some(chain)) => TlsServerConfig::from_path(key, chain).await.map_err(NockAppError::IoError)?,
        (None, None) => TlsServerConfig::default(),
        _ => panic!("Unsupported TLS configuration. Must pass either both --miner-tls-key,--miner-tls-chain, or neither.")
    };

    #[cfg(not(feature = "server-tls-key-load"))]
    let tls = TlsServerConfig::default();

    let tls = Arc::new(tls);

    let mut jwt_keys = vec![];

    #[cfg(feature = "jwt-auth-server")]
    {
        for v in cfg.miner_jwt_keys {
            jwt_keys.push(
                DecodingKey::from_base64_secret(&v)
                    .map_err(|_| NockAppError::OtherError("Invalid JWT".into()))?,
            );
        }
        for i in 1..10 {
            if let Ok(v) = std::env::var(&format!("NBX_JWT_KEY{i}")) {
                jwt_keys.push(
                    DecodingKey::from_base64_secret(&v)
                        .map_err(|_| NockAppError::OtherError("Invalid environment JWT".into()))?,
                );
            }
        }
    }

    let jwt_keys: Arc<[DecodingKey]> = (&*jwt_keys).into();
    let conntrack = ConnTrack::default();
    let conntrack2 = conntrack.clone();

    #[cfg(feature = "db")]
    let db_for_accept = db.clone();
    #[cfg(feature = "compliance")]
    let ip_checker_for_accept = ip_checker.clone();

    #[cfg(feature = "db")]
    let (difficulty_cache, _difficulty_handle) = if let Some(db) = db.as_ref() {
        let (cache, handle) = spawn_difficulty_refresh_task(db.clone()).await;
        (Some(cache), Some(handle))
    } else {
        (None, None)
    };

    #[cfg(feature = "compliance")]
    let (blocklist_cache, blocklist_cache_for_accept, _blocklist_handle) =
        if let Some(db) = db.as_ref() {
            let (cache, handle) = spawn_blocklist_refresh_task(db.clone()).await;
            (Some(cache.clone()), Some(cache), Some(handle))
        } else {
            (None, None, None)
        };

    let (accept_tx, mut accept_rx) = mpsc::channel(8);
    let accept_loop = async move {
        let mut handshake_set = JoinSet::new();
        let mut err_cnt = 0;
        let mut last_iter = Instant::now();
        loop {
            match listener.accept().await {
                Err(e) => {
                    // Client-causable errors.
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                    ) {
                        continue;
                    }
                    // If we keep entering this state, then keep counting up. However, this
                    // shouldn't happen.
                    if last_iter.elapsed() < Duration::from_secs(1) {
                        err_cnt += 1;
                    } else {
                        err_cnt = 1;
                    }
                    if err_cnt > 32 {
                        error!("Accept error {e:?}. Too many accept errors in a row!");
                        return Err(e);
                    }
                    error!("Accept error {e:?}. Sleeping 1 second");
                    sleep(Duration::from_secs(1)).await;
                }
                Ok((s, a)) => {
                    trace!("Accepted {a}");
                    err_cnt = 0;
                    let accept_tx = accept_tx.clone();
                    let jwt_keys = jwt_keys.clone();
                    let tls = tls.clone();
                    let conntrack = conntrack2.clone();

                    // Clone these for each spawn
                    #[cfg(feature = "db")]
                    let db = db_for_accept.clone();
                    #[cfg(feature = "db")]
                    let difficulty_cache = difficulty_cache.clone();
                    #[cfg(feature = "compliance")]
                    let ip_checker = ip_checker_for_accept.clone();
                    #[cfg(feature = "compliance")]
                    let blocklist_cache = blocklist_cache_for_accept.clone();

                    handshake_set.spawn(async move {
                        let s = match tokio::time::timeout(
                            Duration::from_secs(10),
                            tls_accept(s, &tls),
                        )
                        .await
                        {
                            Ok(Ok(v)) => v,
                            Ok(Err(e)) => {
                                debug!("Unable to perform TLS handshake on {a}: {e}");
                                return;
                            }
                            Err(_) => {
                                debug!("Timeout performing TLS handshake on {a}");
                                return;
                            }
                        };

                        let handshake = match tokio::time::timeout(
                            Duration::from_secs(20),
                            server_handshake(
                                s,
                                a,
                                jwt_keys,
                                conntrack,
                                #[cfg(feature = "db")]
                                db.clone(),
                                #[cfg(feature = "compliance")]
                                ip_checker.clone(),
                                #[cfg(feature = "compliance")]
                                blocklist_cache.clone(),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(h)) => h,
                            Ok(Err(e)) => {
                                warn!("Unable to perform NBX handshake on {a}: {e:?}");
                                return;
                            }
                            Err(_) => {
                                warn!("Timeout performing NBX handshake on {a}");
                                return;
                            }
                        };

                        #[cfg(feature = "db")]
                        let bucket_id = difficulty_cache
                            .as_ref()
                            .and_then(|cache| cache.lock().ok())
                            .and_then(|map| map.get(&handshake.client_sub).copied())
                            .unwrap_or(0) as usize;

                        #[cfg(not(feature = "db"))]
                        let bucket_id = 0;

                        if let Err(e) = accept_tx.send((handshake, a, bucket_id)).await {
                            error!("Unable to send accepted connection: {e:?}");
                        }
                    });
                }
            }
            last_iter = Instant::now();
            gauge!("nbx_miner_server_accept_errors_cnt").set(err_cnt as f64);
        }
    };
    client_set.spawn(accept_loop);

    #[cfg(feature = "miner-save-attempts")]
    let save_mine_attempts = cfg.miner_save_attempts.unwrap_or(SaveMineAttempts::None);
    #[cfg(not(feature = "miner-save-attempts"))]
    let save_mine_attempts = SaveMineAttempts::None;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards");
    let run_id = now.as_secs().to_string();
    let mut run_cnt = 0;

    #[cfg(feature = "verifier")]
    let verifier = {
        #[cfg(feature = "force-preverify")]
        let preverify = true;
        #[cfg(not(feature = "force-preverify"))]
        let preverify = cfg.miner_preverify;
        if preverify {
            let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
            let kernel = Vec::from(KERNEL);
            let verifier = SerfThread::<SaveableCheckpoint>::new(
                kernel,
                None,
                hot_state,
                NOCK_STACK_SIZE_TINY,
                vec![],
                Default::default(),
                false,
            )
            .await
            .expect("Could not load mining kernel");
            Some(verifier)
        } else {
            None
        }
    };

    let mut metrics_interval = tokio::time::interval(Duration::from_secs(10));
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.reset();
    let mut conndrop_interval = tokio::time::interval(Duration::from_secs(
        cfg.miner_conndrop_interval_seconds.unwrap_or(60),
    ));
    conndrop_interval.reset();

    // Add periodic blocklist check interval
    let mut blocklist_check_interval = tokio::time::interval(Duration::from_secs(30));
    blocklist_check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let mut reqs_in_futs = reqs_in.iter_mut().map(|v| v.recv()).collect::<Vec<_>>();
        let reqs_in_futs = reqs_in_futs
            .iter_mut()
            // SAFETY: we are dropping the futures after this iteration, so it is okay
            .map(|v| unsafe { core::pin::Pin::new_unchecked(v) })
            .collect::<Vec<_>>();

        tokio::select! {
            v = accept_rx.recv() => {
                let Some((handshake, a, bucket_id)) = v else { continue };
                let sub = handshake.client_sub;
                let hwid = handshake.client_hwid.clone();
                let perms = handshake.perms;
                crate::log!(debug, "Accepted {a} with client_id = {client_cnt}; client_hwid = {hwid}; client_sub = {sub}; bucket_id = {bucket_id}");
                replay_mining_data[bucket_id].retain(|(_, v)| v.get().is_none());
                let cmd = futures::stream::iter(replay_mining_data[bucket_id].clone()).chain(BroadcastStream::new(mining_data_tx[bucket_id].subscribe()).filter_map(|v| async move { v.ok() }));
                let (otx, orx) = oneshot::channel();
                let srv = server(handshake, cmd, client_cnt, tx.clone(), orx);
                let srv = client_set.spawn(srv);
                clients.add(client_cnt, srv, otx, (sub, hwid, perms), sub);
                client_cnt += 1;
            },
            v = client_set.join_next_with_id() => {
                let Some(v) = v else { continue };
                let (id, r) = match v {
                    Ok((id, r)) => (id, Some(r)),
                    Err(e) => (e.id(), None),
                };
                if let Some(client_id) = clients.remove(id) {
                    match r {
                        Some(Err(e)) => crate::log!(debug, "client_id = {client_id} died with error: {e}"),
                        Some(Ok(())) => crate::log!(debug, "client_id = {client_id} died cleanly"),
                        None => crate::log!(warn, "client_id = {client_id} died without finishing"),
                    }
                } else {
                    error!("Accept loop removed");
                    break Err(NockAppError::IoError(io::ErrorKind::BrokenPipe.into()));
                }
            },
            _ = metrics_interval.tick() => {
                let clients = conntrack.emit_metrics();
                if let Some(cc) = connected_clients {
                    cc.store(clients, Ordering::Relaxed);
                }
            },
            _ = conndrop_interval.tick() => {
                if let Some(frac) = cfg.miner_conndrop_fraction {
                    clients.drop_fraction(frac);
                }
            },
            _ = blocklist_check_interval.tick() => {
            #[cfg(all(feature = "db", feature = "compliance"))]
            {
                if let Some(cache) = &blocklist_cache {
                    let connected_subs: Vec<Uuid> = clients.subs.keys().copied().collect();

                    let Ok(blocklisted_subs) = cache.0.lock() else {
                        error!("Blocklist poisoned");
                        continue;
                    };

                    counter!("nbx_miner_server_blocklist_check_total").increment(1);

                    for sub in connected_subs {
                        if blocklisted_subs.contains_key(&sub) {
                            warn!("Disconnecting blocklisted sub: {sub}");
                            let disconnected = clients.abort_by_sub(
                                &sub,
                                AbortReason::Blocklisted
                            );

                            if disconnected > 0 {
                                warn!("Disconnected {disconnected} connections for blocklisted sub: {sub}");
                                counter!(
                                    "nbx_miner_server_blocklist_active_disconnection_total"
                                ).increment(disconnected as u64);
                            }
                        }
                    }
                }}
            },
            data = rx.recv() => {
                let ClientDataRead { data, client_id } = data.expect("Result senders died");

                let Some((sub, hwid, perms)) = clients.lookup(client_id).cloned() else {
                    error!("Unable to lookup client, client_id={client_id}");
                    continue;
                };

                match data {
                    ClientDataReadType::MiningResult(data, in_data) => {
                        let run_cnt_res = run_cnt;
                        run_cnt += 1;

                        trace!("Target hit? {}", data.target_hit);

                        if data.target_hit {
                            let Some((poke, effect_slab)) = data.poke.as_ref().zip(data.effect.as_ref()) else {
                                error!("Successful result without poke and proof");
                                continue;
                            };
                            let effect = unsafe { effect_slab.root() };
                            let Ok(effect) = effect.as_cell().map(|v| v.head()) else {
                                let reason = AbortReason::NounValidation("Expected exactly one effect");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };
                            let Ok([head, res, tail]) = effect.uncell() else {
                                let reason = AbortReason::NounValidation("Expected three elements in mining result");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };
                            if head.eq_bytes("mine-result") {
                                if !unsafe { res.raw_equals(&D(0)) } {
                                    error!("Successful result with improper res value ({res:?})");
                                    continue;
                                }
                            } else {
                                error!("Successful result with improper head");
                                continue;
                            }
                            if save_mine_attempts.should_save_lucky() {
                                save_mine_attempt(&poke, &effect_slab, &run_id, run_cnt_res).await;
                            }
                            let Ok([_, poke]) = tail.uncell() else {
                                let reason = AbortReason::NounValidation("Expected two elements in tail");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };
                            let mut poke_slab = NounSlab::new();
                            poke_slab.copy_into(poke);

                            let Ok([_, _, _, _, _, nonce]) = poke.uncell() else {
                                let reason = AbortReason::NounValidation("Expected 6 elements in the poke result");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };

                            // Verify that the start of the nonce contains the fixed belts
                            let Ok(nonce_noun) = nonce.uncell::<5>() else {
                                let reason = AbortReason::NounValidation("Nonce has invalid number of elements");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };
                            let mut nonce = [Belt(0); 5];
                            let mut cnt = 0;
                            for n in nonce_noun {
                                let Ok(n) = n.as_atom().and_then(|v| v.as_u64()) else {
                                    error!("Nonce has invalid element {n:?}");
                                    break;
                                };
                                nonce[cnt] = Belt(n);
                                cnt += 1;
                            }
                            if cnt != 5 {
                                let reason = AbortReason::NounValidation("Nonce has invalid element");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            }
                            if nonce.iter().zip(in_data.fixed_nonce_atoms.iter()).any(|(a, b)| a != b) {
                                error!("Mined nonce {nonce:?} does not start with fixed belts {:?}", in_data.fixed_nonce_atoms);
                                let reason = AbortReason::NounValidation("Mined nonce does not start with fixed belts");
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            }

                            #[cfg(feature = "verifier")]
                            if let Some(verifier) = &verifier {
                                let mut verif_slab = NounSlab::<NockJammer>::new();
                                let target = unsafe { in_data.target.root() };
                                let verif_poke = T(&mut verif_slab, &[D(tas!(b"verify")), poke, *target, D(in_data.pow_len)]);
                                verif_slab.copy_into(verif_poke);
                                match verifier.poke(MiningWire::Mined.to_wire(), verif_slab).await {
                                    Err(e) => {
                                        error!("Unable to poke verifier {e:?}");
                                        let reason = AbortReason::ValidatorFailure("Unable to poke verifier");
                                        #[cfg(feature = "db")]
                                        db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                        clients.abort(client_id, reason);
                                        continue;
                                    }
                                    Ok(r) => {
                                        let result = unsafe { r.root() };
                                        let Ok(result) = result.as_cell() else {
                                            let reason = AbortReason::ValidatorFailure("Expected result to be a cell");
                                            #[cfg(feature = "db")]
                                            db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                            clients.abort(client_id, reason);
                                            continue;
                                        };
                                        let effect = result.head();
                                        let Ok([outcome, why]) = effect.uncell() else {
                                            let reason = AbortReason::ValidatorFailure("Expected effect to be a tuple");
                                            #[cfg(feature = "db")]
                                            db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                            clients.abort(client_id, reason);
                                            continue;
                                        };

                                        if !outcome.eq_bytes("good") {
                                            let why = why.as_atom().ok();
                                            let why = why.as_ref().map(|v| v.as_ne_bytes()).and_then(|v| std::str::from_utf8(v).ok()).unwrap_or("");
                                            let reason = AbortReason::ProofValidation(why.to_string());
                                            #[cfg(feature = "db")]
                                            db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                            clients.abort(client_id, reason);
                                            continue;
                                        }
                                    }
                                }
                            }

                            if process_target(data, client_id, sub, hwid.clone(), in_data, poke_slab).await.is_err() {
                                let reason = AbortReason::ProofRejected;
                                #[cfg(feature = "db")]
                                db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                                clients.abort(client_id, reason);
                                continue;
                            };
                        } else {
                            trace!("didn't find block, starting new attempt. client={} miner={}", client_id, data.miner_id);
                            let Some((poke, effect)) = data.poke.zip(data.effect) else {
                                continue;
                            };
                            if save_mine_attempts.should_save_unlucky() {
                                save_mine_attempt(&poke, &effect, &run_id, run_cnt_res).await;
                            }
                        }
                    }
                    ClientDataReadType::Telemetry(t) => {
                        if perms.telemetry_metrics {
                            match &t {
                                Telemetry::Proofrate {
                                    machines
                                } => {
                                    for (k, p) in machines {
                                        gauge!(
                                            "nbx_miner_server_telemetry_proofs_per_minute",
                                            "client_sub" => sub.to_string(),
                                            "machine_id" => k.clone(),
                                        ).set(*p as f64);
                                    }
                                }
                                Telemetry::HwInfo {
                                    machines
                                } => {
                                    counter!(
                                        "nbx_miner_server_telemetry_hwinfo_count_total",
                                        "client_sub" => sub.to_string(),
                                    ).increment(machines.len() as u64);
                                }
                            }
                        }

                        if process_telemetry(t, client_id, sub).await.is_err() {
                            let reason = AbortReason::TelemetryError;
                            #[cfg(feature = "db")]
                            db.as_ref().map(|v| v.submit_abort(sub, hwid.clone(), reason.clone()));
                            clients.abort(client_id, reason);
                            continue;
                        }
                    }
                }
            },
            (d, bucket, _) = futures::future::select_all(reqs_in_futs) => {
                let Some((new_mining_data, lock)) = d else { break Ok(()); };
                debug!("received new candidate block header on bucket {bucket}: {:?}",
                    tip5_hash_to_base58(*unsafe { new_mining_data.block_header.root() })
                    .expect("Failed to convert header to Base58")
                );
                if mining_data_tx[bucket].send((new_mining_data.clone(), lock.clone())).is_err() {
                    warn!("No clients connected");
                }
                replay_mining_data[bucket].push_back((new_mining_data, lock));
            }
        }
    }
}
