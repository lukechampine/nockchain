use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use metrics::counter;
use nbx_jetpack::log::{debug, error, info};
use tokio::time::interval;
use tracing::warn;
use uuid::Uuid;

type BlocklistCache = Arc<Mutex<HashSet<Uuid>>>;

pub async fn spawn_blocklist_refresh_task(
    db: crate::db::DatabaseHandle,
) -> (BlocklistCache, tokio::task::JoinHandle<()>) {
    let initial_blocklist = db
        .get_all_blocklisted_subs()
        .await
        .expect("Failed to fetch initial blocklisted subs");

    let cache: BlocklistCache = Arc::new(Mutex::new(initial_blocklist));

    // Spawn background refresh task
    let cache_clone = cache.clone();
    let task_handle = tokio::spawn(async move {
        let mut refresh_interval = interval(Duration::from_secs(60));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            refresh_interval.tick().await;

            if let Some(blocklisted_subs) = db.get_all_blocklisted_subs().await {
                let Ok(mut writer) = cache_clone.lock() else {
                    warn!("Blocklist cache is poisoned");
                    continue;
                };

                *writer = blocklisted_subs;
                debug!("Blocklist cache refreshed with {} entries", writer.len());
            }
        }
    });

    (cache, task_handle)
}
