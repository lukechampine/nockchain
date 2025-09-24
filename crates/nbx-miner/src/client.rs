use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ibig::UBig;
use nbx_jetpack::instruments::ReadInstruments;
use nbx_jetpack::log::*;
use nockapp::nockapp::wire::Wire;
use nockapp::noun::slab::{NockJammer, NounSlab};
use nockapp::noun::NounExt;
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::noun::{Atom, D, T};
use rand::distr::weighted::WeightedIndex;
use rand::prelude::Distribution;
use rand::Rng;
use tokio::sync::mpsc;
use zkvm_jetpack::form::{Belt, PRIME};
use zkvm_jetpack::noun::noun_ext::NounExt as OtherNounExt;

use crate::client_base::{client_loops, ClientConfig, ServerExtras};
use crate::device::{Device, DeviceInfo};
use crate::metrics::{counter, gauge, histogram};
use crate::poker::{PokerAttemptRes, PokerHandle};
use crate::proto::{
    ClientDataWrite, ClientDataWriteType, MiningAckOut, MiningDataOut, MiningResultIn,
};
use crate::server::TELEMETRY_PROOFRATE_INTERVAL;
use crate::shared::{
    digest_to_target, parse_bn, target_to_difficulty, MiningData, MiningResult, MiningWire,
    TargetMetrics, Telemetry,
};

const LOG_TARGET: &str = "nbx::client";

struct MiningRequest {
    data: MiningData,
    last_hit: Instant,
    hit_cnt: usize,
    session_id: u32,
}

fn autodetect_threads(info: &DeviceInfo) -> u64 {
    let mb_per_thread = 1800;
    let ram_threads = info.ram_mb / mb_per_thread;

    let cpu_threads = if info.cpu_count == 1 {
        1
    } else if info.cpu_count < 16 {
        info.cpu_count - 1
    } else {
        info.cpu_count - 2
    };

    let threads = core::cmp::min(ram_threads, cpu_threads);

    if ram_threads < threads {
        crate::log!(debug, "Autodetected {threads} threads. Limited by RAM");
    } else {
        crate::log!(debug, "Autodetected {threads} threads");
    }

    threads
}

pub async fn run_client(cfg: ClientConfig) {
    if cfg.miner_connect.is_empty() {
        crate::log!(error, "miner_connect (--miner-connect) cannot be unset");
        panic!("miner_connect (--miner-connect) cannot be unset")
    }

    let device = Device::new(cfg.client_name, false);
    let hwid = device.hwid.clone();

    if device.info.cpu_arch != device.info.binary_arch {
        crate::log!(
            warn, "Binary architecture ({}) does not match running CPU architecture ({}). Performance or stability may be degraded.",
            device.info.binary_arch, device.info.cpu_arch
        );
    }

    let num_threads = cfg
        .num_threads
        .unwrap_or_else(|| autodetect_threads(&device.info));
    crate::log!(
        info, "Starting NockBox miner {} with {} threads", device.info.binary_version, num_threads
    );

    #[cfg(feature = "gpu")]
    {
        let mut builder = nbx_jetpack::gpu::GpuRegistry::builder();

        for gpu in &cfg.gpus {
            builder = builder
                .add_gpu(
                    gpu.name_filter.as_deref(),
                    gpu.gpu_index,
                    nbx_jetpack::gpu::DEFAULT_GPU_QUEUE_SIZE,
                )
                .unwrap();
        }

        builder.build().unwrap();
    }

    let pin_threads = cfg
        .pin_threads
        .map(|v| v.logical_core_ids(num_threads as _));

    let (mining_attempt_results, mut mining_attempts) = mpsc::channel(num_threads as usize);

    let hot_state = zkvm_jetpack::hot::produce_prover_hot_state();
    let nbx_jets = nbx_jetpack::nbx_jets().collect::<Vec<_>>();
    let hot_state = [nbx_jets, hot_state].concat();
    let test_jets_str = std::env::var("NOCK_TEST_JETS").unwrap_or_default();
    let test_jets = nockapp::kernel::boot::parse_test_jets(test_jets_str.as_str());

    rayon::ThreadPoolBuilder::default()
        .num_threads(num_threads as usize)
        .build_global()
        .expect("Unable to build thread pool");

    let mut miners = tokio::task::JoinSet::new();
    for i in 0..(num_threads as usize) {
        let core_id = pin_threads.as_ref().map(|v| v[i]);

        miners.spawn(PokerHandle::new(
            hot_state.clone(),
            test_jets.clone(),
            i,
            core_id,
            mining_attempt_results.clone(),
            MiningWire::Candidate.to_wire(),
        ));
    }
    let mut miners = miners.join_all().await;
    miners.sort_by_key(|v| v.id());

    let (mining_tx, mut mining_rx) = mpsc::channel(cfg.miner_connect.len());
    let (ack_tx, mut ack_rx) = mpsc::channel(cfg.miner_connect.len());

    let (_client_tasks, server_extras) = client_loops(
        cfg.miner_connect, cfg.miner_num_concurrent_connections, device, mining_tx, ack_tx,
        #[cfg(feature = "jwt-auth-client")]
        cfg.miner_auth_jwt,
        #[cfg(not(feature = "jwt-auth-client"))]
        None,
    );

    let mut requests = BTreeMap::new();

    let mut interval = tokio::time::interval(Duration::from_secs(5));

    #[rustfmt::skip]
    let mut hit_metrics = TargetMetrics::new(Duration::from_secs(10), "hit", "10s")
        .with_previous(TargetMetrics::new(Duration::from_secs(60), "hit", "1m")
        .with_previous(TargetMetrics::new(Duration::from_secs(600), "hit", "10m")
        .with_previous(TargetMetrics::new(Duration::from_secs(3600), "hit", "60m"))));

    #[rustfmt::skip]
    let mut miss_metrics = TargetMetrics::new(Duration::from_secs(10), "miss", "10s")
        .with_previous(TargetMetrics::new(Duration::from_secs(60), "miss", "1m")
        .with_previous(TargetMetrics::new(Duration::from_secs(600), "miss", "10m")
        .with_previous(TargetMetrics::new(Duration::from_secs(3600), "miss", "60m"))));

    #[rustfmt::skip]
    let mut all_metrics = [
        TargetMetrics::new(Duration::from_secs(60), "all", "1m"),
        TargetMetrics::new(Duration::from_secs(600), "all", "10m"),
        TargetMetrics::new(Duration::from_secs(3600), "all", "60m"),
    ];

    let mut counted_proofs = 0;
    let mut telemetry_interval = tokio::time::interval(TELEMETRY_PROOFRATE_INTERVAL);
    telemetry_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));

    stats_interval.reset();

    loop {
        counter!("nbx_miner_client_main_loop_ticks_total").increment(1);

        tokio::select! {
            v = mining_rx.recv() => {
                counter!("nbx_miner_client_main_loop_mining_rx_total").increment(1);
                let MiningDataOut { server_id, session_id, expire, new_datas } = v.expect("Client loop died");
                let max_height = requests.values().map(|v: &MiningRequest| v.data.block_height).max().unwrap_or(0);
                let mut max_height2 = max_height;
                for (data_id, data) in new_datas {
                    max_height2 = core::cmp::max(max_height2, data.block_height);
                    let hit_cnt = requests.values().filter(|v: &&MiningRequest| v.data.block_height == data.block_height).map(|v| v.hit_cnt).min().unwrap_or(0);
                    requests.insert((server_id, data_id), MiningRequest { data, last_hit: Instant::now(), hit_cnt, session_id });
                }
                // NOTE: always expire after inserting requests, because technically, expirations
                // may contain new requests as well (however unlikely).
                for e in expire {
                    requests.remove(&(server_id, e));
                }
                for m in &miners {
                    start_mining_attempt(m, &server_extras, &mut requests);
                }
                if max_height != max_height2 {
                    crate::log!(
                        debug,
                        "Received data with block_height = {max_height2}",
                    );
                }
            }
            v = ack_rx.recv() => {
                counter!("nbx_miner_client_main_loop_ack_rx_total").increment(1);
                let MiningAckOut { server_id, miner_id: _, data_id } = v.expect("Client loop died");
                if let Some(r) = requests.get_mut(&(server_id, data_id)) {
                    histogram!(
                        "nbx_miner_client_ack2ack_seconds",
                        "server_id" => server_id.to_string(),
                    ).record(r.last_hit.elapsed().as_secs_f64());
                    r.last_hit = Instant::now();
                }
            }
            _ = interval.tick() => {
                counter!("nbx_miner_client_main_loop_interval_total").increment(1);
                let max_height = requests.values().map(|v| v.data.block_height).max().unwrap_or(0);
                gauge!(
                    "nbx_miner_client_block_height",
                ).set(max_height as f64);

                let mut live_cnt = 0;

                let tip_cnt = requests
                    .iter()
                    .filter(|((sid, _), _)| if server_extras[*sid].shared().live { live_cnt += 1; true } else { false })
                    .filter(|(_, v)| v.data.block_height == max_height)
                    .count();

                gauge!(
                    "nbx_miner_client_live_servers",
                ).set(live_cnt as f64);

                gauge!(
                    "nbx_miner_client_servers_at_tip",
                ).set(tip_cnt as f64);

                hit_metrics.measure_down();
                miss_metrics.measure_down();
            }
            _ = telemetry_interval.tick() => {
                let telemetry = Telemetry::Proofrate {
                    machines: [(hwid.clone(), counted_proofs)].into_iter().collect(),
                };
                counted_proofs = 0;

                for (server_id, e) in server_extras.iter().enumerate() {
                    let shared = e.shared();
                    if !shared.live || shared.session_id == 0 || !shared.perms.telemetry {
                        continue;
                    }

                    gauge!(
                        "nbx_miner_client_channel_mining_res_capacity",
                        "server_id" => server_id.to_string(),
                    ).set(e.mining_res.capacity() as f64);

                    if let Err(e) = e.mining_res.try_send(
                        ClientDataWrite {
                            session_id: shared.session_id,
                            data: ClientDataWriteType::Telemetry(telemetry.clone()),
                        }
                    ) {
                        counter!(
                            "nbx_miner_client_send_mining_res_fail_total",
                            "server_id" => server_id.to_string(),
                        ).increment(1);
                        error!("Unable to send telemetry to {server_id}: {e:?}");
                    };
                }
            }
            _ = stats_interval.tick() => {
                let mut difficulty_sum = 0;
                let mut cnt = 0;
                if let Some(max_height) = requests
                    .iter()
                    .filter(|((sid, _), _)| server_extras[*sid].shared().live)
                    .inspect(|(_, v)| {
                        let target = unsafe { v.data.target.root() };
                        let target = parse_bn(*target);
                        let diff = target_to_difficulty(target);
                        difficulty_sum += u64::try_from(diff).unwrap_or(u64::MAX);
                        cnt += 1;
                    })
                    .map(|(_, v)| v.data.block_height).max()
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

                let mut stats = vec![];
                for m in &all_metrics {
                    m.get_rate_statistics(&mut stats);
                }
                if !stats.is_empty() {
                    crate::log!(
                        info,
                        "Proofrate: {}",
                        stats.join("; "),
                    );
                }
            }
            r = mining_attempts.recv() => {
                counter!("nbx_miner_client_main_loop_mining_attempts_total").increment(1);
                let PokerAttemptRes { id, duration_millis, inst_delta: ReadInstruments { gpu_enqueue_ms, gpu_submit_ms, gpu_finish_ms }, slab_res, slab_inp, metadata: (server_id, data_id, session_id) } = r.expect("Mining attempt result failed");
                let miner = &miners[id];
                let slab = slab_res.expect("Mining attempt result failed");
                let result = unsafe { slab.root() };
                // If the mining attempt was cancelled, the goof goes into poke_swap which returns
                // %poke followed by the cancelled poke. So we check for hed = %poke
                // to identify a cancelled attempt.
                let effect = result.as_cell().expect("Expected result to be a cell").head();
                if effect.is_cell() {
                    //  there should only be one effect
                    let [head, res, v] = effect.uncell().expect("Expected three elements in mining result");
                    if head.eq_bytes("mine-result") {
                        let (target_hit, poke, effect, dig) = if unsafe { res.raw_equals(&D(0)) } {
                            let [dig, _] = v.uncell().unwrap();
                            (true, Some(slab_inp), Some(slab), dig)
                        } else {
                            (false, None, None, v)
                        };

                        let dig = digest_to_target(dig);

                        counted_proofs += 1;

                        for m in &mut all_metrics {
                            m.measure(dig.clone());
                        }

                        if target_hit {
                            hit_metrics.measure(dig);
                        } else {
                            miss_metrics.measure(dig);
                        }

                        let extra = &server_extras[server_id];

                        gauge!(
                            "nbx_miner_client_channel_mining_res_capacity",
                            "server_id" => server_id.to_string(),
                        ).set(extra.mining_res.capacity() as f64);

                        if target_hit {
                            crate::log!(debug, "Target hit. Submitting");
                            if let Err(e) = extra.mining_res.try_send(
                                ClientDataWrite {
                                    session_id,
                                    data: ClientDataWriteType::MiningResult(MiningResultIn {
                                        data_id,
                                        data: MiningResult {
                                            miner_id: id,
                                            attempt_millis: duration_millis,
                                            gpu_enqueue_millis: gpu_enqueue_ms as _,
                                            gpu_submit_millis: gpu_submit_ms as _,
                                            gpu_wait_millis: gpu_finish_ms as _,
                                            target_hit,
                                            poke,
                                            effect,
                                        }
                                    })
                                }
                            ) {
                                counter!(
                                    "nbx_miner_client_send_mining_res_fail_total",
                                    "server_id" => server_id.to_string(),
                                ).increment(1);
                                error!("Unable to send mining result to {server_id}: {e:?}");
                            }
                        }

                        // TODO: remove all mining requests and wait for new block height to come
                        // in. Also, make sure we receive the target block's height so that we can
                        // filter things out easier.
                    }
                }

                start_mining_attempt(miner, &server_extras, &mut requests);
            }
        }
    }
}

fn create_poke(mining_data: &MiningData, nonce: &NounSlab) -> NounSlab {
    let mut slab = NounSlab::new();
    let header = slab.copy_into(unsafe { *(mining_data.block_header.root()) });
    let version = slab.copy_into(unsafe { *(mining_data.version.root()) });
    let target = slab.copy_into(unsafe { *(mining_data.target.root()) });
    let nonce = slab.copy_into(unsafe { *(nonce.root()) });
    let poke_noun = T(
        &mut slab,
        &[version, header, nonce, target, D(mining_data.pow_len)],
    );
    slab.set_root(poke_noun);
    slab
}

fn start_mining_attempt(
    miner: &PokerHandle<(usize, u32, u32)>,
    server_extras: &[ServerExtras],
    requests: &mut BTreeMap<(usize, u32), MiningRequest>,
) {
    let max_height = requests
        .values()
        .map(|v| v.data.block_height)
        .max()
        .unwrap_or(0);
    gauge!("nbx_miner_client_block_height",).set(max_height as f64);

    let mut live_cnt = 0;

    let mut filtered = requests
        .iter_mut()
        .filter(|((sid, _), _)| server_extras[*sid].shared().live)
        .inspect(|_| live_cnt += 1)
        .filter(|(_, v)| v.data.block_height == max_height)
        .collect::<Vec<_>>();

    gauge!("nbx_miner_client_live_servers",).set(live_cnt as f64);

    if filtered.is_empty() {
        return;
    }

    debug!("live_cnt={live_cnt}, filtered.len()={}", filtered.len());

    let lowest_cnt = filtered.iter().map(|v| v.1.hit_cnt).min().unwrap();
    trace!(
        "lowest_cnt={lowest_cnt}, debug={:?}",
        filtered.iter().map(|v| v.1.hit_cnt).collect::<Vec<_>>()
    );
    let weights = filtered
        .iter()
        .map(|v| 0.5f64.powi((v.1.hit_cnt - lowest_cnt + 1) as i32))
        .collect::<Vec<_>>();
    trace!("weights={weights:?}");

    let index = WeightedIndex::new(weights.iter().copied()).unwrap();

    let mut rng = rand::thread_rng();
    let i = index.sample(&mut rng);

    let (
        (target_sid, data_id),
        MiningRequest {
            data,
            hit_cnt,
            session_id,
            ..
        },
    ) = filtered.swap_remove(i);

    *hit_cnt += 1;

    let mut nonce_slab = NounSlab::<NockJammer>::new();
    let mut nonce_atoms = data.fixed_nonce_atoms.clone();
    while nonce_atoms.len() < 5 {
        nonce_atoms.push(Belt(rng.gen::<u64>() % PRIME));
    }
    let nonce_atoms = nonce_atoms
        .into_iter()
        .map(|v| Atom::new(&mut nonce_slab, v.0).as_noun())
        .collect::<Vec<_>>();
    let nonce_cell = T(&mut nonce_slab, &nonce_atoms);
    nonce_slab.set_root(nonce_cell);
    let nonce = nonce_slab;

    debug!(
        "starting mining attempt on thread {:?} on header {:?} on block {} with nonce: {:?}",
        miner.id(),
        tip5_hash_to_base58(*unsafe { data.block_header.root() })
            .expect("Failed to convert block header to Base58"),
        data.block_height,
        tip5_hash_to_base58(*unsafe { nonce.root() }).expect("Failed to convert nonce to Base58"),
    );
    let poke_slab = create_poke(data, &nonce);
    miner.send_poke(poke_slab, (*target_sid, *data_id, *session_id), false);
}
