use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::net::IpAddr;
use std::num::{NonZero, NonZeroUsize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use lru::LruCache;
use metrics::{counter, gauge};
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::compliance::blocklist::BlocklistCache;
use crate::compliance::ip_address_checks::{IpAddressCheckDecision, IpAddressCheckReason};
use crate::compliance::ipdata::{ConnectionType, IpAddressChecker};

const IP_CHECK_RETRY_COUNT: u8 = 5;
const MAXIMUM_PENDING_IP_COUNT: usize = 10_000;

#[derive(Clone)]
struct IndirectIpTrackerState {
    pending_ips: VecDeque<(Uuid, IpAddr, u8)>,
    checked_ips: LruCache<(Uuid, IpAddr), ()>,
}

#[derive(Clone)]
pub struct IndirectIpTracker {
    state: Arc<Mutex<IndirectIpTrackerState>>,
}

impl IndirectIpTracker {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(IndirectIpTrackerState {
                pending_ips: VecDeque::with_capacity(MAXIMUM_PENDING_IP_COUNT),
                checked_ips: LruCache::new(NonZeroUsize::new(16384).unwrap()),
            })),
        }
    }

    pub fn add_ips(&self, sub: Uuid, ips: impl Iterator<Item = IpAddr>) {
        let mut state = self.state.lock().unwrap();

        gauge!("nbx_miner_indirect_ip_queue_length_cur").set(state.pending_ips.len() as f64);

        for ip in ips {
            if !ip.is_global() {
                continue;
            }

            if state.checked_ips.contains(&(sub, ip)) {
                continue;
            }

            // Don't keep filling the queue if it's not processed fast enough. Eventually, this ip
            //  will be checked either on this proxy or another.
            if state.pending_ips.len() >= MAXIMUM_PENDING_IP_COUNT {
                counter!("nbx_miner_indirect_ip_queue_full_total").increment(1);
                return;
            }

            state.pending_ips.push_back((sub, ip, 1));
        }
    }

    pub fn has_pending_ip(&self) -> bool {
        !self.state.lock().unwrap().pending_ips.is_empty()
    }

    pub fn pop_unchecked_ip(&self) -> Option<(Uuid, IpAddr, u8)> {
        let mut state = self.state.lock().unwrap();

        while let Some((sub, ip, attempt)) = state.pending_ips.pop_front() {
            if state.checked_ips.contains(&(sub, ip)) {
                continue;
            }

            return Some((sub, ip, attempt));
        }

        None
    }

    pub fn mark_checked(&self, sub: Uuid, ip: IpAddr) {
        let mut state = self.state.lock().unwrap();
        state.checked_ips.push((sub, ip), ());
    }

    pub fn reinsert(&self, sub: Uuid, ip: IpAddr, previous_attempts: u8) {
        let mut state = self.state.lock().unwrap();
        state
            .pending_ips
            .push_back((sub, ip, previous_attempts + 1));
    }
}

pub async fn spawn_indirect_connections_ip_check_task(
    db: crate::db::DatabaseHandle,
    ip_checker: IpAddressChecker,
    blocklist: BlocklistCache,
) -> (IndirectIpTracker, tokio::task::JoinHandle<()>) {
    let tracker: IndirectIpTracker = IndirectIpTracker::new();

    let tracker_clone = tracker.clone();
    let task_handle = tokio::spawn(async move {
        let mut refresh_interval = interval(Duration::from_secs(30));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            refresh_interval.tick().await;

            if !tracker.has_pending_ip() {
                continue;
            }

            while let Some((sub, ip, attempt)) = tracker.pop_unchecked_ip() {
                let blocklist_entry = blocklist.check_blocklist(&sub);

                if blocklist_entry.is_block_listed {
                    // Organization is already blocked, no need to check more connections
                    continue;
                }

                info!("Checking ip '{}' (attempt {})", ip, attempt);
                match ip_checker
                    .check_ip(ip, sub, ConnectionType::Indirect, &db)
                    .await
                {
                    None => {
                        warn!("Failed to check IP {ip} for sub={sub}, attempt {attempt}/{IP_CHECK_RETRY_COUNT}");

                        if attempt < IP_CHECK_RETRY_COUNT {
                            tracker.reinsert(sub, ip, attempt);
                        } else {
                            counter!("nbx_miner_proto_ip_block_rejection_total", "client_sub" => sub.to_string()).increment(1);
                            let block_message = "We could not confirm you location, please reach out to support@nockbox.org";

                            // Add the organisation to the block list such that all proxies will disconnect
                            // TODO: Enable for compliance
                            // db.blocklist(sub, block_message.to_string());
                            // blocklist.block(sub, Some(block_message.into()));

                            tracker.mark_checked(sub, ip);
                        }

                        // Add a timeout to prevent overloading the downstream server
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    Some(result) => {
                        if let Some(block_message) = result.block_message() {
                            if blocklist_entry.is_allow_listed {
                                info!("Sub {sub} is allowlisted, ignoring block check decision for {ip}");
                                counter!("nbx_miner_server_allowlist_bypass_total").increment(1);
                            } else {
                                counter!("nbx_miner_server_ip_block_rejection_total", "client_sub" => sub.to_string()).increment(1);

                                // Add the organisation to the block list such that all proxies will disconnect
                                // TODO: Enable for compliance
                                // db.blocklist(sub, block_message.to_string());
                                // blocklist.block(sub, Some(block_message.into()));
                            }
                        }

                        tracker.mark_checked(sub, ip);
                    }
                }
            }
        }
    });

    (tracker_clone, task_handle)
}
