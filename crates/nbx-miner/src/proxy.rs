use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use clap::Args;
use ibig::UBig;
use crate::client_base::client_loops;
use crate::metrics::{counter, gauge, histogram};
use crate::server::{mining_server, MiningConfig};
use nockapp::noun::slab::NounSlab;
use nockapp::{NockAppError, Noun};
use nockvm_macros::tas;
use nockvm::noun::{D, T};
use rustls::crypto::ring::default_provider;
use tokio::sync::mpsc;
use nbx_jetpack::log::*;
use zkvm_jetpack::form::{Belt, PRIME};
use zkvm_jetpack::noun::noun_ext::NounExt as OtherNounExt;

use crate::proto::{MiningAckOut, MiningDataOut, MiningResultIn, RECENTLY_EXPIRED_DURATION};
use crate::shared::{MiningData, MiningResult, TimeWriter};

#[derive(Clone, Debug, Args)]
pub struct ProxyConfig {
    #[command(flatten)]
    server: MiningConfig,
    #[arg(
        long,
        help = "Which servers to connect to in order to receive mining requests from",
        value_delimiter = ','
    )]
    pub miner_connect: Vec<SocketAddr>,
    #[cfg(not(feature = "force-tls"))]
    #[arg(long, help = "Use TLS for the miner")]
    miner_connect_tls: bool,
    #[arg(
        long,
        help = "What's the client name to send in the protocol"
    )]
    pub client_name: String,
    #[arg(long, help = "Whether to forward non-block proofs upstream")]
    pub forward_non_block: bool,
    #[arg(long, help = "Target time to adjust proxy difficulty to", default_value = "60")]
    pub target_share_seconds: u64,
    #[arg(long, help = "Minimum difficulty for the proxy", default_value = "1")]
    pub min_share_difficulty: u64,
}

fn max_target() -> UBig {
    let p = UBig::from(PRIME);
    let p1: UBig = p.clone() - 1;
    let mut max_target = p1.clone();
    for i in 1..=4 {
        max_target += p1.clone() * p.pow(i);
    }
    max_target
}

fn target_to_difficulty(target: UBig) -> UBig {
    max_target() / target
}

fn difficulty_to_target(diff: u64) -> UBig {
    max_target() / diff
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
        // skip the %bn tag
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

fn to_bn(mut v: UBig) -> NounSlab {
    let mut ints = vec![D(tas!(b"bn"))];
    let zero = UBig::from(0u32);
    while v != zero {
        let int = u32::try_from(&v & !0u32).unwrap();
        ints.push(D(int as u64));
        v >>= 32;
    }
    ints.push(D(0));
    let mut slab = NounSlab::new();
    let bn = T(&mut slab, &ints);
    slab.copy_into(bn);
    slab
}

const ROLLING_TIMING_CNT: usize = 20;
const DIFF_ADJUSTMENT_PRC: u64 = 20;

struct DifficultyTracker {
    // Current difficulty multiplied by 10
    current_diff10: u64,
    min_difficulty: u64,
    current_target: UBig,
    rolling_timings: VecDeque<(Instant, u64)>,
    accumulated_work: u64,
    target_interval: Duration,
    last_updated: Instant,
}

impl DifficultyTracker {
    pub fn measure_and_update(&mut self, diff: u64) {
        // Track timings for proxy difficulty adjustments
        if self.rolling_timings.len() >= ROLLING_TIMING_CNT {
            self.rolling_timings.pop_front();
        }
        self.accumulated_work += diff;
        self.rolling_timings.push_back((Instant::now(), self.accumulated_work));
        // Update proxy difficulty
        if self.rolling_timings.len() == ROLLING_TIMING_CNT {
            self.update_difficulty();
        }
    }

    pub fn update_difficulty(&mut self) {
        self.last_updated = Instant::now();
        let (elapsed, accum_work) = if self.rolling_timings.len() < 2 {
            return;
        } else {
            let (start, start_work) = self.rolling_timings.front().unwrap();
            let end = self.last_updated;
            let elapsed = end.duration_since(*start).as_millis();
            (elapsed, self.accumulated_work - start_work)
        };
        trace!("all elapsed: {:?}", self.rolling_timings.iter().map(|v| self.last_updated.duration_since(v.0).as_millis()).collect::<Vec<_>>());
        trace!("{elapsed} * {} / ({accum_work} * 10)", self.current_diff10);
        let elapsed = elapsed * self.current_diff10 as u128 / (accum_work * 10) as u128;
        trace!("{} * {} / {}", self.current_diff10, self.target_interval.as_millis(), elapsed);
        let target_diff10 = self.current_diff10 as u128 * self.target_interval.as_millis() / elapsed;
        let target_diff10 = core::cmp::min(std::u64::MAX as u128, target_diff10) as u64;
        let new_diff10 = (self.current_diff10 * (100 - DIFF_ADJUSTMENT_PRC) + (target_diff10 * DIFF_ADJUSTMENT_PRC)) / 100;
        let new_diff10 = core::cmp::max(new_diff10, 10 * core::cmp::max(self.min_difficulty, 1));
        trace!("Update difficulty {} -> {} ({})", self.current_diff10 / 10, new_diff10 / 10, target_diff10 / 10);
        self.current_diff10 = new_diff10;
        self.current_target = difficulty_to_target(self.current_diff10 / 10);
    }
}

pub async fn run_proxy(cfg: ProxyConfig) {
    let server_listener = crate::server::bind(&cfg.server).await.expect("Unable to bind proxy listener");

    #[cfg(not(feature = "force-tls"))]
    let tls = if cfg.miner_connect_tls {
        let _ = default_provider().install_default();
        Some(Default::default())
    } else {
        None
    };

    #[cfg(feature = "force-tls")]
    let tls = {
        let _ = default_provider().install_default();
        Some(Default::default())
    };

    let (mining_tx, mut mining_rx) = mpsc::channel(cfg.miner_connect.len());
    let (ack_tx, mut ack_rx) = mpsc::channel(cfg.miner_connect.len());
    let (_client_tasks, server_extras) = client_loops(cfg.miner_connect, tls, &cfg.client_name, mining_tx, ack_tx, Default::default());

    let mut requests = BTreeMap::new();
    let server_id_map = SyncMutex::new(BTreeMap::<_, (_, (u32, usize, u32, UBig))>::new());
    let forward_non_block = cfg.forward_non_block;
    let mut last_updated = Instant::now();

    let diff_tracker = Arc::new(SyncMutex::new(DifficultyTracker {
        current_diff10: cfg.min_share_difficulty * 10,
        min_difficulty: cfg.min_share_difficulty,
        current_target: difficulty_to_target(cfg.min_share_difficulty),
        accumulated_work: 0,
        target_interval: Duration::from_secs(cfg.target_share_seconds),
        rolling_timings: Default::default(),
        last_updated,
    }));

    // Any pokes that passed server's verification steps.
    let process_target = |mut data: MiningResult, _, cn: Arc<str>, in_data: Arc<MiningData>, poke_slab: NounSlab| {
        let data_info = server_id_map.lock().unwrap().get(&Arc::as_ptr(&in_data)).map(|(_, v)| (v.clone(), server_extras[v.1].mining_res.clone()));
        let diff_tracker = diff_tracker.clone();
        async move {
            let Some(((data_id, server_id, session_id, parent_target), mining_res)) = data_info else {
                debug!("Unable to grab data info (data expired?)");
                // The data had expired before this function being called.
                return Ok(());
            };

            let proxy_target = unsafe { in_data.target.root() };
            let proxy_target = parse_bn(*proxy_target);
            let proxy_diff = target_to_difficulty(proxy_target);
            let Ok(proxy_diff) = u64::try_from(&proxy_diff) else {
                error!("Too high of difficulty: {proxy_diff}");
                return Err(NockAppError::PokeFailed);
            };

            {
                diff_tracker.lock().unwrap().measure_and_update(proxy_diff);
            }

            counter!("nbx_miner_proxy_global_accumulated_work").increment(proxy_diff);
            counter!(
                "nbx_miner_proxy_accumulated_work",
                "client_cn" => cn,
                "server_id" => server_id.to_string(),
            ).increment(proxy_diff);

            let poke = unsafe { poke_slab.root() };
            let [_, _, _, dig, _, _] = poke.uncell()?;
            let dig = dig.as_atom()?;
            #[cfg(target_endian = "little")]
            let dig = UBig::from_le_bytes(&dig.as_ne_bytes());
            #[cfg(target_endian = "big")]
            let dig = UBig::from_be_bytes(&dig.as_ne_bytes());

            // Local server has verified that we hit the pool target. Now, we need to verify
            // whether we hit the parent target.
            data.target_hit = dig <= parent_target;
            if !data.target_hit {
                if !forward_non_block {
                    return Ok(());
                }
                data.poke = None;
                data.effect = None;
            }

            gauge!(
                "nbx_miner_proxy_channel_mining_res_capacity",
                "server_id" => server_id.to_string(),
            ).set(mining_res.capacity() as f64);

            if let Err(e) = mining_res.try_send(MiningResultIn {
                data_id,
                session_id,
                data,
            }) {
                counter!(
                    "nbx_miner_proxy_send_mining_res_fail_total",
                    "server_id" => server_id.to_string(),
                ).increment(1);
                error!("Unable to send mining result to {server_id}: {e:?}");
                return Err(NockAppError::PokeFailed);
            }

            Ok(())
        }
    };

    let (reqs_out, reqs_in) = mpsc::channel(1);
    let server = mining_server(cfg.server, server_listener, reqs_in, process_target);

    let main_iter = async {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            counter!("nbx_miner_proxy_main_loop_ticks_total").increment(1);

            tokio::select! {
                v = mining_rx.recv() => {
                    counter!("nbx_miner_proxy_main_loop_mining_rx_total").increment(1);
                    let MiningDataOut { server_id, session_id, expire, new_datas } = v.expect("Client loop died");
                    for (data_id, mut data) in new_datas {
                        assert!(data.fixed_nonce_atoms.len() < 4, "fixed_nonce_atoms have length {}. We cannot have this many hops in-between (do we have a routing loop?)", data.fixed_nonce_atoms.len());
                        data.fixed_nonce_atoms.push(Belt(rand::random::<u64>() % PRIME));
                        let parent_target = parse_bn(*unsafe {data.target.root() });
                        trace!("parent_diff={}", target_to_difficulty(parent_target.clone()));
                        {
                            data.target = to_bn(core::cmp::max(parent_target.clone(), diff_tracker.lock().unwrap().current_target.clone()));
                        }
                        let data = Arc::new(data);
                        let (inst_handle, inst) = TimeWriter::new();
                        requests.insert((server_id, data_id), (data.clone(), Instant::now(), inst_handle, session_id));
                        server_id_map.lock().unwrap().insert(Arc::as_ptr(&data), (inst.clone(), (data_id, server_id, session_id, parent_target)));
                        if reqs_out.send((data, inst)).await.is_err() {
                            error!("Failed to send to reqs_out");
                            break;
                        }
                    }
                    // We need to expire after inserting to tracker
                    // However, unlike with proxy difficulty adjustment, here we immediately expire
                    // the data.
                    let mut immediate_expire = vec![];
                    for e in expire {
                        if let Some(r) = requests.remove(&(server_id, e)) {
                            immediate_expire.push(r.0);
                        }
                    }
                    let mut server_id_guard = server_id_map.lock().unwrap();
                    for e in immediate_expire {
                        server_id_guard.remove(&Arc::as_ptr(&e));
                    }
                }
                v = ack_rx.recv() => {
                    counter!("nbx_miner_proxy_main_loop_ack_rx_total").increment(1);
                    let MiningAckOut { server_id, miner_id: _, data_id } = v.expect("Client loop died");
                    if let Some(r) = requests.get_mut(&(server_id, data_id)) {
                        histogram!(
                            "nbx_miner_proxy_ack2ack_seconds",
                            "server_id" => server_id.to_string(),
                        ).record(r.1.elapsed().as_secs_f64());
                        r.1 = Instant::now();
                    }
                }
                _ = interval.tick() => {
                    counter!("nbx_miner_proxy_main_loop_interval_total").increment(1);
                    let max_height = requests.values().map(|v| v.0.block_height).max().unwrap_or(0);
                    gauge!(
                        "nbx_miner_proxy_block_height",
                    ).set(max_height as f64);

                    // Maintain only live servers
                    requests.retain(|(sid, _), _| server_extras[*sid].live.load(Ordering::SeqCst));
                    {
                        server_id_map.lock().unwrap().retain(|_, (v, _)| v.get().filter(|v| v.elapsed() > RECENTLY_EXPIRED_DURATION).is_none());
                    }

                    let mut guard = diff_tracker.lock().unwrap();
                    gauge!(
                        "nbx_miner_proxy_difficulty",
                    ).set((guard.current_diff10 / 10) as f64);
                    // Update 50% over target interval to not interfere with proof based updates
                    // that much.
                    if guard.last_updated.elapsed() >= guard.target_interval * 3 / 2 {
                        guard.update_difficulty();
                    }

                    if last_updated != guard.last_updated {
                        last_updated = guard.last_updated;
                        let new_target = guard.current_target.clone();
                        debug!("Update difficulty to min {}", target_to_difficulty(new_target.clone()));
                        core::mem::drop(guard);
                        let mut new_reqs = BTreeMap::new();
                        for (k, (data, ack_cnt, _inst_handle, session_id)) in requests {
                            let mut server_id_guard = server_id_map.lock().unwrap();
                            let Some((_, (data_id, server_id, session_id2, parent_target))) = server_id_guard.get(&Arc::as_ptr(&data)).cloned() else {
                                error!("Cannot lookup server_id");
                                continue;
                            };
                            let target = to_bn(core::cmp::max(parent_target.clone(), new_target.clone()));
                            assert_eq!(session_id, session_id2);
                            let data = Arc::new(MiningData {
                                block_header: data.block_header.clone(),
                                version: data.version.clone(),
                                target,
                                pow_len: data.pow_len,
                                block_height: data.block_height,
                                fixed_nonce_atoms: data.fixed_nonce_atoms.clone(),
                            });
                            let (inst_handle, inst) = TimeWriter::new();
                            server_id_guard.insert(Arc::as_ptr(&data), (inst.clone(), (data_id, server_id, session_id, parent_target)));
                            core::mem::drop(server_id_guard);
                            new_reqs.insert(k, (data.clone(), ack_cnt, inst_handle, session_id));
                            if reqs_out.send((data, inst)).await.is_err() {
                                error!("Failed to send to reqs_out");
                                break;
                            }
                        }
                        debug!("Difficulty updated");
                        requests = new_reqs;
                    }

                    let tip_cnt = requests
                        .iter()
                        .filter(|(_, v)| v.0.block_height == max_height)
                        .count();

                    let mut prev_srv = None;
                    let live_cnt = requests
                        .keys()
                        .filter(|(s, _)| if Some(*s) != prev_srv {
                            prev_srv = Some(*s);
                            true
                        } else {
                            false
                        })
                        .count();

                    gauge!(
                        "nbx_miner_proxy_live_servers",
                    ).set(live_cnt as f64);

                    gauge!(
                        "nbx_miner_proxy_live_datas",
                    ).set(requests.len() as f64);

                    gauge!(
                        "nbx_miner_proxy_datas_at_tip",
                    ).set(tip_cnt as f64);
                }
            }
        }
    };

    let _ = tokio::join!(server, main_iter);
}
