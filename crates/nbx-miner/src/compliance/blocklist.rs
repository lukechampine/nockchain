use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use metrics::counter;
use thiserror::Error;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct BlocklistCache(Arc<Mutex<BlocklistCacheState>>);

type Blocklist = BTreeMap<Uuid, Option<Arc<str>>>;
type Allowlist = BTreeSet<Uuid>;

#[derive(Clone)]
pub struct BlocklistCacheState {
    pub block_list: Blocklist,
    pub allow_list: Allowlist,
}

pub struct BlocklistEntry {
    pub is_block_listed: bool,
    pub is_allow_listed: bool,
    pub block_message: Option<Arc<str>>,
}

impl BlocklistCache {
    pub fn check_blocklist(&self, sub: &Uuid) -> BlocklistEntry {
        self.0.lock().unwrap().check_blocklist(sub)
    }

    pub fn filter_blocked_subs(&self, subs: &[Uuid]) -> Vec<Uuid> {
        self.0.lock().unwrap().filter_blocked_subs(subs)
    }

    pub fn update(&self, blocklist: Option<Blocklist>, allowlist: Option<Allowlist>) {
        self.0.lock().unwrap().update(blocklist, allowlist);
    }

    pub fn block(&self, sub: Uuid, message: Option<Arc<str>>) {
        self.0.lock().unwrap().block(sub, message);
    }
}

impl BlocklistCacheState {
    fn is_allow_listed(&self, sub: &Uuid) -> bool {
        self.allow_list.contains(sub)
    }

    fn is_block_listed(&self, sub: &Uuid) -> bool {
        self.block_list.contains_key(sub)
    }

    fn is_blocked(&self, sub: &Uuid) -> bool {
        if !self.is_block_listed(sub) {
            return false;
        }

        if self.is_allow_listed(sub) {
            return false;
        }

        true
    }

    pub fn check_blocklist(&self, sub: &Uuid) -> BlocklistEntry {
        let message = self.block_list.get(sub);
        let is_block_listed = message.is_some();
        let is_allow_listed = self.is_allow_listed(sub);

        BlocklistEntry {
            is_block_listed,
            is_allow_listed,
            block_message: message.cloned().unwrap_or(None),
        }
    }

    pub fn filter_blocked_subs(&self, subs: &[Uuid]) -> Vec<Uuid> {
        subs.iter()
            .copied()
            .filter(|sub| self.is_blocked(sub))
            .collect()
    }

    pub fn block(&mut self, sub: Uuid, message: Option<Arc<str>>) {
        self.block_list.entry(sub).or_insert(message);
    }

    pub fn update(&mut self, blocklist: Option<Blocklist>, allowlist: Option<Allowlist>) {
        if let Some(blocklist) = blocklist {
            self.block_list = blocklist;
            debug!(
                "Blocklist cache refreshed with {} entries",
                self.block_list.len()
            );
        }

        if let Some(allowlist) = allowlist {
            self.allow_list = allowlist;
            debug!(
                "Allowlist cache refreshed with {} entries",
                self.allow_list.len()
            );
        }
    }
}

pub async fn spawn_blocklist_refresh_task(
    db: crate::db::DatabaseHandle,
) -> (BlocklistCache, tokio::task::JoinHandle<()>) {
    let initial_blocklist = db
        .get_all_blocklisted_subs()
        .await
        .expect("Failed to fetch initial blocklisted subs");

    let initial_allowlist = db
        .get_all_allowlisted_subs()
        .await
        .expect("Failed to fetch initial blocklisted subs");

    let cache = BlocklistCache(Arc::new(Mutex::new(BlocklistCacheState {
        block_list: initial_blocklist,
        allow_list: initial_allowlist,
    })));

    // Spawn background refresh task
    let cache_clone = cache.clone();
    let task_handle = tokio::spawn(async move {
        let mut refresh_interval = interval(Duration::from_secs(60));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            refresh_interval.tick().await;

            let blocklisted_subs = db.get_all_blocklisted_subs().await;
            let allowlisted_subs = db.get_all_allowlisted_subs().await;
            cache_clone.update(blocklisted_subs, allowlisted_subs);
        }
    });

    (cache, task_handle)
}
