use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use clap_serde_derive::ClapSerde;
use ibig::UBig;
use nbx_jetpack::log::*;
use nockapp::noun::slab::NounSlab;
use nockapp::NockAppError;
use nockchain_math::belt::{Belt, PRIME};
use nockchain_math::noun_ext::NounMathExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::client_base::client_loops;
#[cfg(feature = "compliance")]
use crate::compliance::ipdata::IpAddressChecker;
use crate::device::{Device, DeviceInfoWithSockets};
use crate::metrics::{counter, gauge, histogram};
use crate::proto::{
    get_recently_expired_duration, ClientDataWrite, ClientDataWriteType, MiningAckOut,
    MiningDataOut, MiningResultIn, RECENTLY_EXPIRED_DURATION,
};
use crate::server::{mining_server, MiningConfig, TELEMETRY_PROOFRATE_INTERVAL};
#[cfg(feature = "verifier")]
use crate::shared::difficulty_to_target;
use crate::shared::{
    digest_to_target, parse_bn, target_to_difficulty, to_bn, MiningData, MiningResult,
    TargetMetrics, Telemetry, TimeWriter,
};

const LOG_TARGET: &str = "nbx::proxy";

#[cfg(feature = "verifier")]
pub const NUM_DIFF_BUCKETS: usize = 2;
#[cfg(not(feature = "verifier"))]
pub const NUM_DIFF_BUCKETS: usize = 1;

#[derive(ClapSerde, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[arg(
        long,
        help = "Which servers to connect to in order to receive mining requests from",
        value_delimiter = ','
    )]
    pub miner_connect: Vec<String>,
    #[default(3)]
    #[arg(
        long,
        help = "How many concurrent connections to maintain [default: 3]"
    )]
    pub miner_num_concurrent_connections: usize,
    #[arg(
        long,
        help = "What's the client name to send in the protocol. Affects machine ID."
    )]
    pub client_name: Option<String>,
    #[default(60)]
    #[arg(long, help = "Target time to adjust proxy difficulty to [default: 60]")]
    #[cfg(feature = "verifier")]
    pub target_share_seconds: u64,
    #[default(1)]
    #[arg(long, help = "Minimum difficulty for the proxy [default: 1]")]
    #[cfg(feature = "verifier")]
    pub min_share_difficulty: u64,
    #[arg(
        long,
        help = "Whether to forward telemetry (hardware info + proofrate)",
        default_value = "true"
    )]
    pub miner_telemetry: bool,
    #[cfg(feature = "db")]
    #[arg(long, help = "URL to database. e.g.: postgres://localhost:3243/pool")]
    pub database_url: Option<String>,
    #[cfg(feature = "jwt-auth-client")]
    #[arg(
        long,
        help = "JWT to use in order to authenticate to servers. Overrides NBX_AUTH_JWT environment variable."
    )]
    pub miner_auth_jwt: Option<Arc<str>>,
}

const ROLLING_TIMING_CNT: usize = 20;
const DIFF_ADJUSTMENT_PRC: u64 = 20;

#[cfg(feature = "verifier")]
#[derive(Clone)]
struct DifficultyTracker {
    // Current difficulty multiplied by 10
    current_diff10: u64,
    min_difficulty: u64,
    current_target: UBig,
    rolling_timings: std::collections::VecDeque<(Instant, u64)>,
    accumulated_work: u64,
    target_interval: Duration,
    last_updated: Instant,
    update_cnt: usize,
}

#[cfg(feature = "verifier")]
impl DifficultyTracker {
    pub fn measure_and_update(&mut self, diff: u64) {
        // Track timings for proxy difficulty adjustments
        if self.rolling_timings.len() >= ROLLING_TIMING_CNT {
            self.rolling_timings.pop_front();
        }
        self.accumulated_work += diff;
        self.rolling_timings
            .push_back((Instant::now(), self.accumulated_work));
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
        trace!(
            "all elapsed: {:?}",
            self.rolling_timings
                .iter()
                .map(|v| self.last_updated.duration_since(v.0).as_millis())
                .collect::<Vec<_>>()
        );
        trace!("{elapsed} * {} / ({accum_work} * 10)", self.current_diff10);
        let elapsed = elapsed * self.current_diff10 as u128 / (accum_work * 10) as u128;
        trace!(
            "{} * {} / {}",
            self.current_diff10,
            self.target_interval.as_millis(),
            elapsed
        );
        let target_diff10 =
            self.current_diff10 as u128 * self.target_interval.as_millis() / elapsed;
        let target_diff10 = core::cmp::min(std::u64::MAX as u128, target_diff10) as u64;
        let new_diff10 = (self.current_diff10 * (100 - DIFF_ADJUSTMENT_PRC)
            + (target_diff10 * DIFF_ADJUSTMENT_PRC))
            / 100;
        let new_diff10 = core::cmp::max(new_diff10, 10 * core::cmp::max(self.min_difficulty, 1));
        trace!(
            "Update difficulty {} -> {} ({})",
            self.current_diff10 / 10,
            new_diff10 / 10,
            target_diff10 / 10
        );
        self.current_diff10 = new_diff10;
        self.current_target = difficulty_to_target(self.current_diff10 / 10);
        self.update_cnt += 1;
    }
}

pub async fn run_proxy(cfg: ProxyConfig, server_cfg: MiningConfig) {
    let server_listener = crate::server::bind(&server_cfg)
        .await
        .expect("Unable to bind proxy listener");

    let device = Device::new(cfg.client_name.clone(), true);

    if cfg.miner_connect.is_empty() {
        eprintln!("miner_connect (--miner-connect) cannot be unset");
        std::process::exit(1);
    }

    crate::log!(
        info,
        "Starting NockBox proxy {} on {}",
        device.info.binary_version,
        server_cfg.miner_bind()
    );

    #[cfg(feature = "db")]
    let (db_inst, db) = if let Some(db) = cfg.database_url {
        debug!("Connecting to DB");
        crate::db::Database::new(
            &db,
            cfg.client_name
                .as_deref()
                .or(device.info.hostname.as_deref())
                .map(|v| Arc::<str>::from(v))
                .unwrap_or_else(|| device.hwid.clone()),
        )
        .await
        .map(|(a, b)| (Some(a), Some(b)))
        .unwrap()
    } else {
        debug!("Skipping DB connection");
        (None, None)
    };

    let (mining_tx, mut mining_rx) = mpsc::channel(cfg.miner_connect.len());
    let (ack_tx, mut ack_rx) = mpsc::channel(cfg.miner_connect.len());
    let (_client_tasks, server_extras) = client_loops(
        cfg.miner_connect, cfg.miner_num_concurrent_connections, device, mining_tx, ack_tx,
        #[cfg(feature = "jwt-auth-client")]
        cfg.miner_auth_jwt,
        #[cfg(not(feature = "jwt-auth-client"))]
        None,
    );

    let mut requests = (0..NUM_DIFF_BUCKETS)
        .map(|_| BTreeMap::new())
        .collect::<Vec<_>>();
    let server_id_map =
        SyncMutex::new(BTreeMap::<_, (_, (u32, usize, u32, UBig, usize, usize))>::new());

    #[cfg(feature = "verifier")]
    let mut last_updated = vec![Instant::now(); NUM_DIFF_BUCKETS];
    #[cfg(feature = "verifier")]
    let mut update_cnt = vec![0; NUM_DIFF_BUCKETS];
    #[cfg(feature = "verifier")]
    let diff_tracker = Arc::new(SyncMutex::new(vec![
        DifficultyTracker {
            current_diff10: cfg.min_share_difficulty * 10,
            min_difficulty: cfg.min_share_difficulty,
            current_target: difficulty_to_target(cfg.min_share_difficulty),
            accumulated_work: 0,
            target_interval: Duration::from_secs(cfg.target_share_seconds),
            rolling_timings: Default::default(),
            last_updated: last_updated[0],
            update_cnt: update_cnt[0],
        };
        NUM_DIFF_BUCKETS
    ]));

    #[rustfmt::skip]
    let hit_metrics = TargetMetrics::new(Duration::from_secs(10), "hit", "10s")
        .with_previous(TargetMetrics::new(Duration::from_secs(60), "hit", "1m")
        .with_previous(TargetMetrics::new(Duration::from_secs(600), "hit", "10m")
        .with_previous(TargetMetrics::new(Duration::from_secs(3600), "hit", "60m"))));

    #[rustfmt::skip]
    let miss_metrics = TargetMetrics::new(Duration::from_secs(10), "miss", "10s")
        .with_previous(TargetMetrics::new(Duration::from_secs(60), "miss", "1m")
        .with_previous(TargetMetrics::new(Duration::from_secs(600), "miss", "10m")
        .with_previous(TargetMetrics::new(Duration::from_secs(3600), "miss", "60m"))));
    let hit_metrics = Arc::new(SyncMutex::new(hit_metrics));
    let miss_metrics = Arc::new(SyncMutex::new(miss_metrics));

    // Any pokes that passed server's verification steps.
    let process_target = |mut data: MiningResult,
                          _,
                          sub: Uuid,
                          hwid: Arc<str>,
                          in_data: Arc<MiningData>,
                          poke_slab: NounSlab| {
        // Do not hold the lock
        let data_info = {
            server_id_map
                .lock()
                .unwrap()
                .get(&Arc::as_ptr(&in_data))
                .map(|(_, v)| (v.clone(), server_extras[v.1].mining_res.clone()))
        };
        #[cfg(feature = "verifier")]
        let diff_tracker = diff_tracker.clone();
        let hit_metrics = hit_metrics.clone();
        let miss_metrics = miss_metrics.clone();
        #[cfg(feature = "db")]
        let db = db.clone();
        async move {
            let Some((
                (data_id, server_id, session_id, parent_target, diff_bucket_id, _),
                mining_res,
            )) = data_info
            else {
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

            #[cfg(feature = "verifier")]
            {
                diff_tracker.lock().unwrap()[diff_bucket_id].measure_and_update(proxy_diff);
            }

            counter!("nbx_miner_proxy_global_accumulated_work", "diff_bucket_id" => diff_bucket_id.to_string()).increment(proxy_diff);
            counter!(
                "nbx_miner_proxy_accumulated_work",
                "client_sub" => sub.to_string(),
                "server_id" => server_id.to_string(),
                "diff_bucket_id" => diff_bucket_id.to_string(),
            )
            .increment(proxy_diff);

            let poke = unsafe { poke_slab.root() };
            let [_, _, _, dig, _, _] = poke.uncell()?;
            let dig = dig.as_atom()?;
            #[cfg(target_endian = "little")]
            let dig = UBig::from_le_bytes(&dig.as_ne_bytes());
            #[cfg(target_endian = "big")]
            let dig = UBig::from_be_bytes(&dig.as_ne_bytes());

            #[cfg(feature = "db")]
            db.as_ref().map(|v| {
                v.submit_share(
                    sub.clone(),
                    hwid.clone(),
                    dig.clone(),
                    proxy_diff,
                    in_data.block_height,
                )
            });

            // Local server has verified that we hit the pool target. Now, we need to verify
            // whether we hit the parent target.
            data.target_hit = dig <= parent_target;
            if !data.target_hit {
                miss_metrics.lock().unwrap().measure(dig);
                return Ok(());
            } else {
                hit_metrics.lock().unwrap().measure(dig);
            }

            crate::log!(debug, "{sub} on {hwid} hit parent target. Forwarding.");

            gauge!(
                "nbx_miner_proxy_channel_mining_res_capacity",
                "server_id" => server_id.to_string(),
            )
            .set(mining_res.capacity() as f64);

            if let Err(e) = mining_res.try_send(ClientDataWrite {
                session_id,
                data: ClientDataWriteType::MiningResult(MiningResultIn { data_id, data }),
            }) {
                counter!(
                    "nbx_miner_proxy_send_mining_res_fail_total",
                    "server_id" => server_id.to_string(),
                )
                .increment(1);
                error!("Unable to send mining result to {server_id}: {e:?}");
                return Err(NockAppError::PokeFailed);
            }

            Ok(())
        }
    };

    #[derive(Default)]
    struct TelemetryStore {
        hit_count: BTreeMap<Uuid, usize>,
        proofrate: BTreeMap<Uuid, BTreeMap<Arc<str>, u32>>,
        hwinfo: BTreeMap<Uuid, BTreeMap<Arc<str>, DeviceInfoWithSockets>>,
    }

    let telemetry = SyncMutex::new(TelemetryStore::default());

    let process_telemetry = |in_telemetry, _, client_sub| {
        let mut tl = telemetry.lock().unwrap();
        match in_telemetry {
            Telemetry::Proofrate { machines } => {
                tl.proofrate.entry(client_sub).or_default().extend(machines);
            }
            Telemetry::HwInfo { machines } => {
                tl.hwinfo.entry(client_sub).or_default().extend(machines);
            }
        }
        async move { Ok(()) }
    };

    let connected_clients = AtomicUsize::new(0);

    let mut reqs_out = vec![];
    let mut reqs_in = vec![];
    for _ in 0..NUM_DIFF_BUCKETS {
        let (ro, ri) = mpsc::channel(1);
        reqs_out.push(ro);
        reqs_in.push(ri);
    }
    let server = mining_server(
        server_cfg,
        server_listener,
        reqs_in,
        process_target,
        process_telemetry,
        Some(&connected_clients),
        #[cfg(feature = "db")]
        db.clone(),
        #[cfg(feature = "compliance")]
        Some(IpAddressChecker::from_env()),
    );

    let main_iter = async {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut telemetry_interval = tokio::time::interval(TELEMETRY_PROOFRATE_INTERVAL);
        telemetry_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        #[cfg(feature = "db")]
        let db = db.clone();
        let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
        stats_interval.reset();

        loop {
            counter!("nbx_miner_proxy_main_loop_ticks_total").increment(1);

            tokio::select! {
                v = mining_rx.recv() => {
                    counter!("nbx_miner_proxy_main_loop_mining_rx_total").increment(1);
                    let MiningDataOut { server_id, session_id, expire, new_datas } = v.expect("Client loop died");
                    'fullout: for (data_id, mut data) in new_datas {
                        let hop_count = data.fixed_nonce_atoms.len();
                        assert!(hop_count < 4, "We are at {} hops. This is too many (do we have a routing loop?)", hop_count);
                        data.fixed_nonce_atoms.push(Belt(rand::random::<u64>() % PRIME));
                        let parent_target = parse_bn(*unsafe {data.target.root() });
                        trace!("parent_diff={}", target_to_difficulty(parent_target.clone()));
                        #[cfg(feature = "verifier")]
                        let targets = {
                            let mut tracker = diff_tracker.lock().unwrap();
                            let mut targets = vec![];
                            for i in 0..NUM_DIFF_BUCKETS {
                                targets.push(to_bn(core::cmp::max(parent_target.clone(), tracker[i].current_target.clone())));
                            }
                            targets
                        };
                        #[cfg(not(feature = "verifier"))]
                        let targets = [to_bn(parent_target.clone())];
                        for (i, target) in targets.into_iter().enumerate() {
                            data.target = target;
                            let data = Arc::new(data.clone());
                            let (inst_handle, inst) = TimeWriter::new();
                            requests[i].insert((server_id, data_id), (data.clone(), Instant::now(), inst_handle, session_id, hop_count));
                            server_id_map.lock().unwrap().insert(
                                Arc::as_ptr(&data),
                                (inst.clone(), (data_id, server_id, session_id, parent_target.clone(), i, hop_count))
                            );
                            if reqs_out[i].send((data, inst)).await.is_err() {
                                error!("Failed to send to reqs_out");
                                break 'fullout;
                            }
                        }
                    }
                    // We need to expire after inserting to tracker
                    for e in expire {
                        for i in 0..NUM_DIFF_BUCKETS {
                            requests[i].remove(&(server_id, e));
                        }
                    }
                }
                v = ack_rx.recv() => {
                    counter!("nbx_miner_proxy_main_loop_ack_rx_total").increment(1);
                    let MiningAckOut { server_id, miner_id: _, data_id } = v.expect("Client loop died");
                    if let Some(r) = requests[0].get_mut(&(server_id, data_id)) {
                        histogram!(
                            "nbx_miner_proxy_ack2ack_seconds",
                            "server_id" => server_id.to_string(),
                        ).record(r.1.elapsed().as_secs_f64());
                        r.1 = Instant::now();
                    }
                }
                _ = telemetry_interval.tick() => {
                    counter!("nbx_miner_proxy_main_loop_telemetry_interval_total").increment(1);
                    let mut tl = telemetry.lock().unwrap();
                    let keys = tl.proofrate.keys().chain(tl.hwinfo.keys()).copied().collect::<BTreeSet<_>>();
                    // 1 minute per 1k machine per sub, but only up to 10 minutes.
                    let target_hc = |l: usize| {
                        core::cmp::min(core::cmp::max(1, l / 1024), 10)
                    };
                    let mut proofrate = BTreeMap::new();
                    let mut hwinfo = BTreeMap::new();
                    for k in &keys {
                        let hc = tl.hit_count.entry(*k).or_default();
                        *hc += 1;
                        let hc = *hc;
                        if tl.proofrate.get(k).map(|v| hc % target_hc(v.len()) == 0).unwrap_or(false) {
                            let (a, b) = tl.proofrate.remove_entry(k).unwrap();
                            proofrate.insert(a, b);
                        }
                        if tl.hwinfo.get(k).map(|v| hc % target_hc(v.len()) == 0).unwrap_or(false) {
                            let (a, b) = tl.hwinfo.remove_entry(k).unwrap();
                            hwinfo.insert(a, b);
                        }
                    }
                    let mut out_tl = vec![];
                    if !proofrate.is_empty() {
                        #[cfg(feature = "db")]
                        db.as_ref().map(|v| v.submit_telemetry_proofrate(proofrate.clone()));
                        // In case there are duplicate machine IDs, yes, we are merging them together.
                        let machines = proofrate.into_values().flatten().collect::<BTreeMap<_, _>>();
                        out_tl.push(Telemetry::Proofrate { machines });
                    }
                    if !hwinfo.is_empty() {
                        #[cfg(feature = "db")]
                        db.as_ref().map(|v| v.submit_telemetry_hwinfo(hwinfo.clone()));
                        // In case there are duplicate machine IDs, yes, we are merging them together.
                        let machines = hwinfo.into_values().flatten().collect::<BTreeMap<_, _>>();
                        out_tl.push(Telemetry::HwInfo { machines });
                    }

                    for tl in out_tl {
                        for (server_id, e) in server_extras.iter().enumerate() {
                            let shared = e.shared();
                            if !shared.live || shared.session_id == 0 || !shared.perms.telemetry || !cfg.miner_telemetry {
                                continue;
                            }

                            gauge!(
                                "nbx_miner_proxy_channel_mining_res_capacity",
                                "server_id" => server_id.to_string(),
                            ).set(e.mining_res.capacity() as f64);

                            if let Err(e) = e.mining_res.try_send(
                                ClientDataWrite {
                                    session_id: shared.session_id,
                                    data: ClientDataWriteType::Telemetry(tl.clone()),
                                }
                            ) {
                                counter!(
                                    "nbx_miner_proxy_send_mining_res_fail_total",
                                    "server_id" => server_id.to_string(),
                                ).increment(1);
                                error!("Unable to send telemetry to {server_id}: {e:?}");
                            };
                        }
                    }
                }
                _ = stats_interval.tick() => {
                    let mut difficulty_sum = 0;
                    let mut cnt = 0;
                    if let Some(max_height) = requests
                        .iter()
                        .flatten()
                        .filter(|((sid, _), _)| server_extras[*sid].shared().live)
                        .inspect(|(_, v)| {
                            let target = unsafe { v.0.target.root() };
                            let target = parse_bn(*target);
                            let diff = target_to_difficulty(target);
                            difficulty_sum += u64::try_from(diff).unwrap_or(u64::MAX);
                            cnt += 1;
                        })
                        .map(|(_, v)| v.0.block_height).max()
                    {
                        let difficulty_avg = difficulty_sum / cnt;
                        crate::log!(
                            info,
                            "Connected. block_height = {max_height}; difficulty = {difficulty_avg}"
                        )
                    } else {
                        crate::log!(
                            warn,
                            "Waiting for connection."
                        );
                    }

                    crate::log!(debug, "Connected clients: {}", connected_clients.load(Ordering::Relaxed));
                }
                _ = interval.tick() => {
                    counter!("nbx_miner_proxy_main_loop_interval_total").increment(1);
                    let max_height = requests.iter().flat_map(|v| v.values()).map(|v| v.0.block_height).max().unwrap_or(0);
                    gauge!(
                        "nbx_miner_proxy_block_height",
                    ).set(max_height as f64);

                    // Maintain only live servers
                    requests.iter_mut().for_each(|v| v.retain(|(sid, _), _| server_extras[*sid].shared().live));
                    {
                        server_id_map.lock().unwrap().retain(|ptr, (v, (_, _, _, _, _, hop_count))| {
                            let duration = get_recently_expired_duration(*hop_count);
                            v.get().filter(|v| v.elapsed() > duration).is_none()
                        });
                    }

                    #[cfg(feature = "verifier")]
                    {
                        // NOTE: we absolutely cannot await while holding this lock, because we can
                        // end up deadlocking.
                        let mut guard = diff_tracker.lock().unwrap();
                        let mut new_reqs_vec = (0..NUM_DIFF_BUCKETS).map(|_| BTreeMap::new()).collect::<Vec<_>>();
                        let mut reqs_out_list = vec![];
                        for (((bucket_id, tracker), reqs), new_reqs) in guard.iter_mut().enumerate().zip(requests).zip(&mut new_reqs_vec) {
                            gauge!(
                                "nbx_miner_proxy_difficulty",
                                "diff_bucket_id" => bucket_id.to_string(),
                            ).set((tracker.current_diff10 / 10) as f64);
                            // Update 50% over target interval to not interfere with proof based updates
                            // that much.
                            if tracker.last_updated.elapsed() >= tracker.target_interval * 3 / 2 {
                                tracker.update_difficulty();
                            }

                            if tracker.last_updated.duration_since(last_updated[bucket_id]) >= RECENTLY_EXPIRED_DURATION * 3 || tracker.update_cnt - update_cnt[bucket_id] >= 5 {
                                last_updated[bucket_id] = tracker.last_updated;
                                update_cnt[bucket_id] = tracker.update_cnt;
                                let new_target = tracker.current_target.clone();
                                debug!("Update difficulty of bucket {bucket_id} to min {}", target_to_difficulty(new_target.clone()));
                                for (k, (data, ack_cnt, _inst_handle, session_id, hop_count)) in reqs {
                                    let mut server_id_guard = server_id_map.lock().unwrap();
                                    let Some((_, (data_id, server_id, session_id2, parent_target, stored_bucket_id, _))) = server_id_guard.get(&Arc::as_ptr(&data)).cloned() else {
                                        error!("Cannot lookup server_id");
                                        continue;
                                    };
                                    assert_eq!(bucket_id, stored_bucket_id);
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
                                    server_id_guard.insert(Arc::as_ptr(&data), (inst.clone(), (data_id, server_id, session_id, parent_target, bucket_id, hop_count)));
                                    core::mem::drop(server_id_guard);
                                    new_reqs.insert(k, (data.clone(), ack_cnt, inst_handle, session_id, hop_count));
                                    reqs_out_list.push((data, inst, bucket_id));
                                }
                                debug!("Difficulty updated on bucket {bucket_id}");
                            } else {
                                *new_reqs = reqs;
                            }
                        }
                        requests = new_reqs_vec;
                        core::mem::drop(guard);
                        for (data, inst, bucket_id) in reqs_out_list {
                            if reqs_out[bucket_id].send((data, inst)).await.is_err() {
                                error!("Failed to send to reqs_out");
                                break;
                            }
                        }
                    }

                    let tip_cnt = requests
                        .iter()
                        .flatten()
                        .filter(|(_, v)| v.0.block_height == max_height)
                        .count();

                    let mut prev_srv = None;
                    let live_cnt = requests
                        .iter()
                        .flat_map(|v| v.keys())
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

                    for (i, req) in requests.iter().enumerate() {
                        gauge!(
                            "nbx_miner_proxy_live_datas",
                            "diff_bucket_id" => i.to_string()
                        ).set(req.len() as f64);
                    }

                    gauge!(
                        "nbx_miner_proxy_datas_at_tip",
                    ).set(tip_cnt as f64);

                    hit_metrics.lock().unwrap().measure_down();
                    miss_metrics.lock().unwrap().measure_down();
                }
            }
        }
    };

    let main = async move {
        tokio::select! {
            r = server => {
                error!("Server finished: {r:?}");
            },
            _ = main_iter => unreachable!(),
        }
        let _ = process_target;
    };

    #[cfg(feature = "db")]
    tokio::join!(main, async move {
        if let Some(db_inst) = db_inst {
            db_inst.run().await;
        }
    },);
    #[cfg(not(feature = "db"))]
    main.await;
}
