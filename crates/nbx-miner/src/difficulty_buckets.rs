use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use metrics::counter;
use nbx_jetpack::log::{debug, error, info};
use tokio::time::interval;
use tracing::warn;
use uuid::Uuid;

type DifficultyBucketAssignmentCache = Arc<Mutex<HashMap<Uuid, i32>>>;

pub async fn spawn_difficulty_refresh_task(
    db: crate::db::DatabaseHandle,
) -> (DifficultyBucketAssignmentCache, tokio::task::JoinHandle<()>) {
    let initial_bucket_assignments = db
        .get_all_difficulty_bucket_assignments()
        .await
        .unwrap_or_default();

    let cache: DifficultyBucketAssignmentCache = Arc::new(Mutex::new(initial_bucket_assignments));

    // Spawn background refresh task
    let cache_clone = cache.clone();
    let task_handle = tokio::spawn(async move {
        let mut refresh_interval = interval(Duration::from_secs(5 * 60));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            refresh_interval.tick().await;

            if let Some(bucket_assignments) = db.get_all_difficulty_bucket_assignments().await {
                let Ok(mut writer) = cache_clone.lock() else {
                    warn!("Difficulty cache is poisoned");
                    continue;
                };

                *writer = bucket_assignments;
                info!("Difficulty cache refreshed with {} entries", writer.len());
            }
        }
    });

    (cache, task_handle)
}
