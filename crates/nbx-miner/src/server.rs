use std::collections::{BTreeMap, HashMap};
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Args;
use nockapp::driver::NockAppHandle;
use nockapp::noun::slab::NounSlab;
use nockapp::wire::Wire;
use nockapp::{NockAppError, NounExt};
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::noun::D;
use rustls::crypto::ring::default_provider;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::{AbortHandle, Id, JoinSet};
use tokio::time::sleep;
use tracing::*;
use zkvm_jetpack::noun::noun_ext::NounExt as ZNounExt;

use crate::proto::{server, MiningResultOut};
use crate::shared::{tls_accept_wrap, MiningData, MiningWire, TlsServerConfig};

#[derive(Clone, Debug, Args)]
pub struct MiningConfig {
    #[arg(
        long,
        help = "Where to bind the mining server to",
        default_value = "[::1]:0"
    )]
    miner_bind: SocketAddr,
    #[arg(long, help = "Use TLS for the miner")]
    miner_bind_tls: bool,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            miner_bind: (Ipv6Addr::LOCALHOST, 0).into(),
            miner_bind_tls: false,
        }
    }
}

type Result<T = ()> = core::result::Result<T, NockAppError>;

pub async fn bind(cfg: &MiningConfig) -> Result<TcpListener> {
    if cfg.miner_bind_tls {
        let _ = default_provider().install_default();
    }

    let listener = TcpListener::bind(cfg.miner_bind)
        .await
        .map_err(NockAppError::IoError)?;
    info!(
        "Bound on {}",
        listener.local_addr().map_err(NockAppError::IoError)?
    );
    Ok(listener)
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SaveMineAttempts {
    #[default]
    None,
    All,
    Blocks,
}

impl SaveMineAttempts {
    fn should_save_lucky(self) -> bool {
        matches!(self, Self::All | Self::Blocks)
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
struct Clients {
    handles: BTreeMap<usize, AbortHandle>,
    ids: HashMap<Id, usize>,
}

impl Clients {
    fn abort(&mut self, client: usize) {
        debug!("Aborting client_id={client}");
        self.handles.get(&client).unwrap().abort();
    }

    fn remove(&mut self, id: Id) -> Option<usize> {
        let client = self.ids.remove(&id)?;
        self.handles.remove(&client);
        Some(client)
    }

    fn add(&mut self, client: usize, handle: AbortHandle) {
        self.ids.insert(handle.id(), client);
        self.handles.insert(client, handle);
    }
}

pub async fn mining_driver(
    cfg: MiningConfig,
    listener: TcpListener,
    handle: NockAppHandle,
) -> Result {
    let (tx, mut rx) = mpsc::channel(1024);
    let mining_data_tx = broadcast::channel(16).0;
    let cur_mining_data = Mutex::new(None);

    let mut client_set = JoinSet::new();
    let mut clients = Clients::default();
    let mut client_cnt = 0;

    let tls = if cfg.miner_bind_tls {
        Some(TlsServerConfig::default())
    } else {
        None
    };

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
                Ok((s, a)) => {
                    trace!("Accepted {a}");
                    err_cnt = 0;
                    if accept_tx.send((s, a)).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    };
    client_set.spawn(accept_loop);

    let save_mine_attempts = SaveMineAttempts::Blocks;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards");
    let run_id = now.as_secs().to_string();
    let mut run_cnt = 0;

    loop {
        tokio::select! {
            v = accept_rx.recv() => {
                let Some((s, a)) = v else { continue };
                debug!("Accepted client_id={client_cnt} on {a}");
                let cmd = { cur_mining_data.lock().await.clone() };
                let srv = server(s, mining_data_tx.subscribe(), cmd, client_cnt, tx.clone());
                let srv = client_set.spawn(srv);
                clients.add(client_cnt, srv);
                client_cnt += 1;
            },
            v = client_set.join_next_with_id() => {
                let Some(v) = v else { continue };
                let id = match v {
                    Ok((id, _)) => id,
                    Err(e) => e.id(),
                };
                if let Some(client_id) = clients.remove(id) {
                    error!("client_id={client_id} died");
                } else {
                    error!("Accept loop removed");
                    break;
                }
            }
            data = rx.recv() => {
                let MiningResultOut { data, client_id, .. } = data.expect("Result senders died");
                let run_cnt_res = run_cnt;
                run_cnt += 1;

                if data.is_block {
                    let Some((poke, effect_slab)) = data.poke.zip(data.effect) else {
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
                    info!("Found block! client={} miner={}", client_id, data.miner_id);
                    let Ok([_, poke]) = tail.uncell() else {
                        error!("Expected two elements in tail. client_id={client_id}");
                        clients.abort(client_id);
                        continue;
                    };
                    let mut poke_slab = NounSlab::new();
                    poke_slab.copy_into(poke);
                    if handle.poke(MiningWire::Mined.to_wire(), poke_slab).await.is_err() {
                        error!("Could not poke nockchain with mined PoW. client_id={client_id}");
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
                    let (version_slab, header_slab, target_slab, pow_len) = {
                        let [version, commit, target, pow_len_noun] = effect_cell.tail().uncell().expect(
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
                        (version_slab, header_slab, target_slab, pow_len)
                    };
                    debug!("received new candidate block header: {:?}",
                        tip5_hash_to_base58(*unsafe { header_slab.root() })
                        .expect("Failed to convert header to Base58")
                    );

                    let new_mining_data = Arc::new(MiningData {
                        block_header: header_slab,
                        version: version_slab,
                        target: target_slab,
                        pow_len: pow_len
                    });

                    let mut guard = cur_mining_data.lock().await;

                    if guard.as_ref() != Some(&new_mining_data) {
                        debug!("Creating new mining attempts");
                        if mining_data_tx.send(new_mining_data.clone()).is_err() {
                            warn!("No clients connected");
                        }
                        *guard = Some(new_mining_data);
                    }
                }
            }
        }
    }

    Ok(())
}
