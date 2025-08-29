use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::pin::pin;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Args, ValueEnum};
#[cfg(feature = "verifier")]
use kernels::verifier::KERNEL;
#[cfg(feature = "verifier")]
use nockapp::{
    kernel::form::SerfThread,
    save::SaveableCheckpoint,
    utils::NOCK_STACK_SIZE_TINY,
    noun::slab::NockJammer,
};
#[cfg(feature = "verifier")]
use nockvm::noun::T;
#[cfg(feature = "verifier")]
use nockvm_macros::tas;
#[cfg(feature = "verifier")]
use tokio::sync::Mutex;
use nockapp::driver::NockAppHandle;
use nockapp::noun::slab::NounSlab;
use nockapp::wire::Wire;
use nockapp::{NockAppError, NounExt};
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::noun::D;
use rustls::crypto::ring::default_provider;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio::task::{AbortHandle, Id, JoinSet};
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;
use nbx_jetpack::log::*;
use zkvm_jetpack::form::{Belt, PRIME};
use zkvm_jetpack::noun::noun_ext::NounExt as ZNounExt;
use futures::stream::StreamExt;

use crate::proto::{server, MiningResultOut};
use crate::shared::{tls_accept_wrap, MiningData, MiningResult, MiningWire, TimeWriter, TlsServerConfig};

#[derive(Clone, Debug, Args)]
pub struct MiningConfig {
    #[arg(
        long,
        help = "Where to bind the mining server to",
        default_value = "[::1]:0"
    )]
    miner_bind: SocketAddr,
    #[cfg(not(feature = "force-tls"))]
    #[arg(long, help = "Use TLS for the miner")]
    miner_bind_tls: bool,
    #[cfg(all(feature = "verifier", not(feature = "force-preverify")))]
    miner_preverify: bool,
    #[arg(long, help = "Which mining attempts to save", default_value = "none")]
    miner_save_attempts: SaveMineAttempts,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            miner_bind: (Ipv6Addr::LOCALHOST, 0).into(),
            #[cfg(not(feature = "force-tls"))]
            miner_bind_tls: false,
            #[cfg(all(feature = "verifier", not(feature = "force-preverify")))]
            miner_preverify: cfg!(feature = "force-preverify"),
            miner_save_attempts: Default::default(),
        }
    }
}

type Result<T = ()> = core::result::Result<T, NockAppError>;

pub async fn bind(cfg: &MiningConfig) -> Result<TcpListener> {
    #[cfg(not(feature = "force-tls"))]
    if cfg.miner_bind_tls {
        let _ = default_provider().install_default();
    }
    #[cfg(feature = "force-tls")]
    let _ = default_provider().install_default();

    let listener = TcpListener::bind(cfg.miner_bind)
        .await
        .map_err(NockAppError::IoError)?;
    info!(
        "Bound on {}",
        listener.local_addr().map_err(NockAppError::IoError)?
    );
    Ok(listener)
}

#[derive(Default, Clone, Copy, PartialEq, Eq, ValueEnum, Debug)]
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

#[derive(Default)]
pub(crate) struct Clients<M> {
    handles: BTreeMap<usize, (AbortHandle, M)>,
    ids: HashMap<Id, usize>,
}

impl<M> Clients<M> {
    pub fn abort(&mut self, client: usize) {
        debug!("Aborting client_id={client}");
        self.handles.get(&client).unwrap().0.abort();
    }

    pub fn remove(&mut self, id: Id) -> Option<usize> {
        let client = self.ids.remove(&id)?;
        self.handles.remove(&client);
        Some(client)
    }

    pub fn add(&mut self, client: usize, handle: AbortHandle, metadata: M) {
        self.ids.insert(handle.id(), client);
        self.handles.insert(client, (handle, metadata));
    }

    pub fn lookup(&self, client: usize) -> Option<&M> {
        self.handles.get(&client).map(|(_, v)| v)
    }
}

pub async fn mining_driver(
    cfg: MiningConfig,
    listener: TcpListener,
    handle: NockAppHandle,
) -> Result {
    let (reqs_out, reqs_in) = mpsc::channel(1);

    let process_target = |data: MiningResult, client_id: usize, cn: Arc<str>, _, poke_slab: NounSlab| {
        info!("Found block! client={} cn={cn} miner={}", client_id, data.miner_id);
        let fut = handle.poke(MiningWire::Mined.to_wire(), poke_slab);
        async move {
            fut.await.map(|_| ())
        }
    };

    let server = mining_server(cfg, listener, reqs_in, process_target);
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

pub async fn mining_server<F: FnMut(MiningResult, usize, Arc<str>, Arc<MiningData>, NounSlab) -> Fut, Fut: Future<Output = Result>>(
    cfg: MiningConfig,
    listener: TcpListener,
    mut reqs_in: mpsc::Receiver<(Arc<MiningData>, Arc<OnceLock<Instant>>)>,
    mut process_target: F,
) -> Result {
    let (tx, mut rx) = mpsc::channel(1024);
    let mining_data_tx = broadcast::channel(16).0;
    let mut replay_mining_data = VecDeque::<(Arc<MiningData>, Arc<OnceLock<Instant>>)>::new();

    let mut client_set = JoinSet::new();
    let mut clients = Clients::default();
    let mut client_cnt = 0;

    #[cfg(not(feature = "force-tls"))]
    let tls = if cfg.miner_bind_tls {
        Some(TlsServerConfig::default())
    } else {
        None
    };
    #[cfg(feature = "force-tls")]
    let tls = Some(TlsServerConfig::default());

    let (accept_tx, mut accept_rx) = mpsc::channel(8);
    let accept_loop = async move {
        let mut err_cnt = 0;
        loop {
            match tls_accept_wrap(listener.accept(), tls).await {
                Err(e) => {
                    // TODO: ignore errors causable by clients
                    err_cnt += 1;
                    if err_cnt > 32 {
                        error!("Accept error {e:?}. Too many accept errors in a row!");
                        return Err(e);
                    }
                    error!("Accept error {e:?}. Sleeping 1 second");
                    sleep(Duration::from_secs(1)).await;
                }
                Ok((s, a, cn)) => {
                    trace!("Accepted {a}");
                    err_cnt = 0;
                    if accept_tx.send((s, a, cn)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    };
    client_set.spawn(accept_loop);

    let save_mine_attempts = cfg.miner_save_attempts;
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
                false,
            )
            .await
            .expect("Could not load mining kernel");
            Some(verifier)
        } else {
            None
        }
    };

    loop {
        tokio::select! {
            v = accept_rx.recv() => {
                let Some((s, a, cn)) = v else { continue };
                debug!("Accepted client_id={client_cnt} with cn={cn:?} on {a}");
                replay_mining_data.retain(|(_, v)| v.get().is_none());
                let cmd = futures::stream::iter(replay_mining_data.clone()).chain(BroadcastStream::new(mining_data_tx.subscribe()).filter_map(|v| async move { v.ok() }));
                let srv = server(s, cmd, client_cnt, cn.clone(), tx.clone());
                let srv = client_set.spawn(srv);
                clients.add(client_cnt, srv, cn);
                client_cnt += 1;
            },
            v = client_set.join_next_with_id() => {
                let Some(v) = v else { continue };
                let (id, r) = match v {
                    Ok((id, r)) => (id, Some(r)),
                    Err(e) => (e.id(), None),
                };
                if let Some(client_id) = clients.remove(id) {
                    error!("client_id={client_id} died ({r:?})");
                } else {
                    error!("Accept loop removed");
                    break;
                }
            }
            data = rx.recv() => {
                let MiningResultOut { data, client_id, in_data, .. } = data.expect("Result senders died");

                let Some(cn) = clients.lookup(client_id).cloned() else {
                    error!("Unable to lookup client, client_id={client_id}");
                    continue;
                };

                let run_cnt_res = run_cnt;
                run_cnt += 1;

                debug!("Target hit? {}", data.target_hit);

                if data.target_hit {
                    let Some((poke, effect_slab)) = data.poke.as_ref().zip(data.effect.as_ref()) else {
                        error!("Successful result without poke and proof");
                        continue;
                    };
                    let effect = unsafe { effect_slab.root() };
                    let Ok(effect) = effect.as_cell().map(|v| v.head()) else {
                        error!("Expected exactly one effect, client_id={client_id}");
                        clients.abort(client_id);
                        continue;
                    };
                    let Ok([head, res, tail]) = effect.uncell() else {
                        error!("Expected three elements in mining result, client_id={client_id}");
                        clients.abort(client_id);
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
                        error!("Expected two elements in tail. client_id={client_id}");
                        clients.abort(client_id);
                        continue;
                    };
                    let mut poke_slab = NounSlab::new();
                    poke_slab.copy_into(poke);

                    let Ok([_, _, _, _, _, nonce]) = poke.uncell() else {
                        error!("Expected 6 elements in the poke result.");
                        clients.abort(client_id);
                        continue;
                    };

                    // Verify that the start of the nonce contains the fixed belts
                    let Ok(nonce_noun) = nonce.uncell::<5>() else {
                        error!("Nonce has invalid number of elements");
                        clients.abort(client_id);
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
                        clients.abort(client_id);
                        continue;
                    }
                    if nonce.iter().zip(in_data.fixed_nonce_atoms.iter()).any(|(a, b)| a != b) {
                        error!("Mined nonce {nonce:?} does not start with fixed belts {:?}", in_data.fixed_nonce_atoms);
                        clients.abort(client_id);
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
                                error!("Unable to poke verifier poke {e:?}");
                                clients.abort(client_id);
                                continue;
                            }
                            Ok(r) => {
                                let result = unsafe { r.root() };
                                let Ok(result) = result.as_cell() else {
                                    error!("Expected result to be a cell");
                                    clients.abort(client_id);
                                    continue;
                                };
                                let effect = result.head();
                                let Ok([outcome, why]) = effect.uncell() else {
                                    error!("Expected effect to be a tuple");
                                    clients.abort(client_id);
                                    continue;
                                };

                                if !outcome.eq_bytes("good") {
                                    let why = why.as_atom().ok();
                                    let why = why.as_ref().map(|v| v.as_ne_bytes()).and_then(|v| std::str::from_utf8(v).ok()).unwrap_or("");
                                    error!("Verifier did not return good result. Why: {why}");
                                    clients.abort(client_id);
                                    continue;
                                }
                            }
                        }
                    }

                    if process_target(data, client_id, cn, in_data, poke_slab).await.is_err() {
                        error!("Mined PoW was not accepted. client_id={client_id}");
                        clients.abort(client_id);
                        continue;
                    };
                } else {
                    debug!("didn't find block, starting new attempt. client={} miner={}", client_id, data.miner_id);
                    let Some((poke, effect)) = data.poke.zip(data.effect) else {
                        continue;
                    };
                    if save_mine_attempts.should_save_unlucky() {
                        save_mine_attempt(&poke, &effect, &run_id, run_cnt_res).await;
                    }
                }
            }
            d = reqs_in.recv() => {
                let Some((new_mining_data, lock)) = d else { break; };
                debug!("received new candidate block header: {:?}",
                    tip5_hash_to_base58(*unsafe { new_mining_data.block_header.root() })
                    .expect("Failed to convert header to Base58")
                );
                if mining_data_tx.send((new_mining_data.clone(), lock.clone())).is_err() {
                    warn!("No clients connected");
                }
                replay_mining_data.push_back((new_mining_data, lock));
            }
        }
    }

    Ok(())
}
