use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;

use ibig::UBig;
use metrics::{counter, gauge};
use nbx_jetpack::log::error;
use nockchain_libp2p_io::tip5_util::ubig_to_base58;
use sqlx::any::install_default_drivers;
use sqlx::postgres::PgPoolOptions as DbPoolOptions;
use sqlx::types::{Json, Uuid};
use sqlx::PgPool as DbPool;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::compliance::ip_address_checks::IpAddressCheckDecision;
use crate::device::DeviceInfoWithSockets;
use crate::server::AbortReason;

enum DbMsg {
    Share {
        client_sub: Uuid,
        machine_id: Arc<str>,
        share_hash: String,
        work_done: u64,
        block_height: u64,
    },
    TelemetryProofrate {
        machines: BTreeMap<Uuid, BTreeMap<Arc<str>, u32>>,
    },
    TelemetryHwinfo {
        machines: BTreeMap<Uuid, BTreeMap<Arc<str>, DeviceInfoWithSockets>>,
    },
    Abort {
        client_sub: Uuid,
        machine_id: Arc<str>,
        reason: AbortReason,
    },
    PartitionMaintenance,
    IpAddressDetails {
        sub: Uuid,
        ip_address: IpAddr,
        decision: IpAddressCheckDecision,
        response: String,
    },
    CheckIpAddress {
        sub: Uuid,
        ip_address: IpAddr,
        response: tokio::sync::oneshot::Sender<Option<IpAddressCheckDecision>>,
    },
    CheckBlocklist {
        sub: Uuid,
        response: tokio::sync::oneshot::Sender<(bool, Option<String>)>,
    },
    Block {
        sub: Uuid,
        message: String,
    },
    CheckAllowlist {
        sub: Uuid,
        response: tokio::sync::oneshot::Sender<bool>,
    },
    GetAllBlocklisted {
        response: tokio::sync::oneshot::Sender<Option<HashSet<Uuid>>>,
    },
    GetAllDifficultyBucketAssignments {
        response: tokio::sync::oneshot::Sender<Option<HashMap<Uuid, i32>>>,
    },
}

pub(crate) struct Database {
    pool: DbPool,
    msgs: Receiver<DbMsg>,
    src_name: Arc<str>,
}

impl Database {
    pub async fn new(db_url: &str, src_name: Arc<str>) -> sqlx::Result<(Self, DatabaseHandle)> {
        install_default_drivers();
        let pool = DbPoolOptions::new().connect(db_url).await?;

        let (msgs_tx, msgs_rx) = channel(4096);

        Ok((
            Self {
                pool,
                msgs: msgs_rx,
                src_name,
            },
            DatabaseHandle { msgs: msgs_tx },
        ))
    }

    pub async fn run(self) {
        let Self {
            pool,
            mut msgs,
            src_name,
        } = self;

        while let Some(msg) = msgs.recv().await {
            let r = match msg {
                DbMsg::Share { client_sub, machine_id, share_hash, work_done, block_height } => {
                    sqlx::query("INSERT INTO \"shares\" (src, sub, machine_id, share_hash, accumulated_work, block_height) VALUES ( $1, $2, $3, $4, $5, $6 )")
                        .bind(&*src_name)
                        .bind(client_sub)
                        .bind(&*machine_id)
                        .bind(share_hash)
                        .bind(i64::try_from(work_done).unwrap_or(i64::MAX))
                        .bind(i64::try_from(block_height).unwrap_or(i64::MAX))
                        .execute(&pool)
                        .await.err()
                }
                DbMsg::TelemetryProofrate {
                    machines
                } => {
                    let query = r#"
                        -- $1 ::uuid[]          -- subs
                        -- $2 ::varchar(8)[]    -- machine_ids
                        -- $3 ::int[]           -- cur values (same length as $2)

                        WITH d AS (
                          SELECT s::uuid AS sub, m::varchar(8) AS machine_id, v::int AS cur
                          FROM unnest($1::uuid[], $2::varchar(8)[], $3::int[]) AS t(s, m, v)
                        )
                        INSERT INTO "aggregate_proofrate" (
                            sub, machine_id,
                            cur_proof_rate, sum_proof_rate, min_proof_rate, max_proof_rate,
                            num_rates
                          )
                        SELECT sub, machine_id, cur, cur, cur, cur, 1
                        FROM d
                        ON CONFLICT (sub, machine_id, created_at_hour)
                        DO UPDATE SET
                          cur_proof_rate = EXCLUDED.cur_proof_rate,
                          sum_proof_rate = aggregate_proofrate.sum_proof_rate + EXCLUDED.sum_proof_rate,
                          min_proof_rate = LEAST(aggregate_proofrate.min_proof_rate, EXCLUDED.min_proof_rate),
                          max_proof_rate = GREATEST(aggregate_proofrate.max_proof_rate, EXCLUDED.max_proof_rate),
                          num_rates = aggregate_proofrate.num_rates + 1,
                          updated_at = NOW();
                    "#;

                    let mut subs = vec![];
                    let mut machine_ids = vec![];
                    let mut proofrates = vec![];

                    for (sub, machines) in &machines {
                        for (machine_id, proofrate) in machines {
                            subs.push(*sub);
                            machine_ids.push(&**machine_id);
                            proofrates.push(*proofrate as i64);
                        }
                    }

                    let chunk_size = 8192;
                    let subs = subs.chunks(chunk_size);
                    let machine_ids = machine_ids.chunks(chunk_size);
                    let proofrates = proofrates.chunks(chunk_size);

                    for res in futures::future::join_all(itertools::izip!(subs, machine_ids, proofrates).map(|(subs, machine_ids, proofrates)| {
                        sqlx::query(&query)
                            .bind(subs)
                            .bind(machine_ids)
                            .bind(proofrates)
                            .execute(&pool)
                    })).await {
                        if let Err(e) = res {
                            error!("Unable to execute query: {e}");
                            counter!("nbx_miner_db_query_errors_total").increment(1);
                        }
                    }

                    None
                }
                DbMsg::TelemetryHwinfo {
                    machines
                } => {
                    // Write to the new table
                    let query = r#"
                        -- $1 ::varchar(255)    -- src
                        -- $2 ::uuid[]          -- subs
                        -- $3 ::varchar(8)[]    -- machine_ids
                        -- $4 ::bool[]          -- is_proxy
                        -- $5 ::jsonb[]         -- hardware

                        WITH d AS (
                          SELECT $1::varchar(255) as src, s::uuid AS sub, m::varchar(8) AS machine_id, p::bool AS is_proxy, h::jsonb AS hardware
                          FROM unnest($2::uuid[], $3::varchar(8)[], $4::bool[], $5::jsonb[]) AS t(s, m, p, h)
                        )
                        INSERT INTO "machines_v2" (
                            src,
                            sub, machine_id,
                            is_proxy, hardware
                          )
                        SELECT src, sub, machine_id, is_proxy, hardware
                        FROM d
                        ON CONFLICT (src, sub, machine_id)
                        DO UPDATE SET
                          is_proxy = EXCLUDED.is_proxy,
                          hardware = EXCLUDED.hardware,
                          updated_at = NOW();
                    "#;

                    let mut subs = vec![];
                    let mut machine_ids = vec![];
                    let mut is_proxy = vec![];
                    let mut hardware = vec![];

                    for (sub, m) in &machines {
                        for (mid, dev) in m {
                            subs.push(sub.clone());
                            machine_ids.push(&**mid);
                            is_proxy.push(dev.device.is_proxy);
                            hardware.push(Json(dev));
                        }
                    }

                    let chunk_size = 4096;
                    let subs = subs.chunks(chunk_size);
                    let machine_ids = machine_ids.chunks(chunk_size);
                    let is_proxy = is_proxy.chunks(chunk_size);
                    let hardware = hardware.chunks(chunk_size);

                    for res in futures::future::join_all(itertools::izip!(subs, machine_ids, is_proxy, hardware).map(|(subs, machine_ids, is_proxy, hardware)| {
                        sqlx::query(&query)
                            .bind(&*src_name)
                            .bind(subs)
                            .bind(machine_ids)
                            .bind(is_proxy)
                            .bind(hardware)
                            .execute(&pool)
                    })).await {
                        if let Err(e) = res {
                            error!("Unable to execute query: {e}");
                            counter!("nbx_miner_db_query_errors_total").increment(1);
                        }
                    }

                    None
                }
                DbMsg::Abort {
                    client_sub,
                    machine_id,
                    reason,
                } => {
                    sqlx::query("INSERT INTO \"aborts\" (src, sub, machine_id, reason) VALUES ( $1, $2, $3, $4 )")
                        .bind(&*src_name)
                        .bind(client_sub)
                        .bind(&*machine_id)
                        .bind(reason.to_string())
                        .execute(&pool)
                        .await.err()
                }
                DbMsg::PartitionMaintenance => {
                    let drop_result = sqlx::query("SELECT * FROM drop_old_proofrate_partitions(2)")
                        .execute(&pool)
                        .await;

                    if let Err(e) = drop_result {
                        error!("Unable to execute query: {e}");
                        counter!("nbx_miner_db_query_errors_total").increment(1);
                    }

                    let create_result = sqlx::query("SELECT ensure_proofrate_partitions()")
                        .execute(&pool)
                        .await;

                    create_result.err()
                }
                DbMsg::CheckBlocklist { sub, response } => {
                    let result = sqlx::query_scalar::<_, Option<String>>(
                        "SELECT block_message
                            FROM block_list
                            WHERE sub = $1
                            AND revoked_at IS NULL
                            ORDER BY created_at DESC
                            LIMIT 1"
                    )
                        .bind(sub)
                        .fetch_optional(&pool)
                        .await;

                    match result.as_ref() {
                        Ok(Some(message)) => {
                            // Row found - sub is blocklisted
                            let _ = response.send((true, message.clone()));
                        }
                        Ok(None) => {
                            // No row found - sub is not blocklisted
                            let _ = response.send((false, None));
                        }
                        Err(_) => {
                            // Database error - handled below by returning the error
                        }
                    }

                    result.err()
                }
                DbMsg::CheckAllowlist { sub, response } => {
                    let result = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS(
                            SELECT 1 FROM allow_list
                            WHERE sub = $1
                            AND revoked_at IS NULL
                        )"
                    )
                        .bind(sub)
                        .fetch_one(&pool)
                        .await;

                    if let Ok(is_allowed) = result.as_ref() {
                        let _ = response.send(*is_allowed);
                    }

                    result.err()
                }
                DbMsg::IpAddressDetails { sub, ip_address, decision, response } => {
                    sqlx::query(
                        "INSERT INTO ip_lookups
                        (id, sub, ip_address, decision, decision_reason, raw_response)
                        VALUES ($1, $2, $3, $4::IPLOOKUPDECISION, $5::IPLOOKUPDECISIONREASON, $6)"
                    )
                        .bind(Uuid::new_v4())
                        .bind(sub)
                        .bind(ip_address)
                        .bind(decision.decision_as_str())
                        .bind(decision.reason_as_str())
                        .bind(sqlx::types::Json(&response))
                        .execute(&pool)
                        .await.err()
                }
                DbMsg::CheckIpAddress { sub, ip_address, response } => {
                        let result = sqlx::query_as::<_, (String, Option<String>)>(
                            "SELECT decision::text, decision_reason::text FROM ip_lookups
                            WHERE sub = $1
                            AND ip_address = $2
                            ORDER BY looked_up_at DESC
                            LIMIT 1"
                        )
                            .bind(sub)
                            .bind(ip_address)
                            .fetch_optional(&pool)
                            .await;

                        if let Ok(row) = result.as_ref() {
                            let decision = row.as_ref().and_then(|(decision_str, reason_str)| {
                                IpAddressCheckDecision::from_str(&decision_str, reason_str.as_deref())
                            });
                            let _ = response.send(decision);
                        }

                        result.err()
                }
                DbMsg::Block { sub, message } => {
                    sqlx::query(
                        "INSERT INTO block_list (sub, block_message)
                            VALUES ($1, $2)"
                    )
                        .bind(sub)
                        .bind(message)
                        .execute(&pool)
                        .await.err()
                }
                DbMsg::GetAllBlocklisted { response } => {
                    let result = sqlx::query_scalar::<_, Uuid>(
                        "SELECT DISTINCT sub
                             FROM block_list
                             WHERE revoked_at IS NULL"
                        )
                        .fetch_all(&pool)
                        .await;

                    match result.as_ref() {
                        Ok(subs) => {
                            let blocklist: HashSet<Uuid> = subs.iter().copied().collect();
                            let _ = response.send(Some(blocklist));
                        }
                        Err(e) => {
                            error!("Failed to fetch blocklist: {e}");
                            // Send empty set on error
                            let _ = response.send(None);
                        }
                    }

                    result.err()
                },
                DbMsg::GetAllDifficultyBucketAssignments { response } => {
                    let result = sqlx::query_as::<_, (Uuid, i32)>(
                        "SELECT sub, bucket_id FROM difficulty_buckets"
                    )
                        .fetch_all(&pool)
                        .await;

                    match result.as_ref() {
                        Ok(rows) => {
                            let buckets: HashMap<Uuid, i32> = rows.iter().copied().collect();
                            let _ = response.send(Some(buckets));
                        }
                        Err(e) => {
                            error!("Failed to fetch difficulty buckets: {e}");
                            let _ = response.send(None);
                        }
                    }

                    result.err()
                }
            };

            if let Some(e) = r {
                error!("Unable to execute query: {e}");
                counter!("nbx_miner_db_query_errors_total").increment(1);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct DatabaseHandle {
    msgs: Sender<DbMsg>,
}

impl DatabaseHandle {
    fn submit_msg(&self, msg: DbMsg) {
        gauge!("nbx_miner_db_msg_channel_capacity_cur").set(self.msgs.capacity() as f64);
        if self.msgs.try_send(msg).is_err() {
            error!("Unable to send message to database loop");
            counter!("nbx_miner_db_submit_error_total").increment(1);
        }
    }

    pub fn submit_share(
        &self,
        client_sub: Uuid,
        machine_id: Arc<str>,
        share_hash: UBig,
        work_done: u64,
        block_height: u64,
    ) {
        let share_hash = ubig_to_base58(share_hash);
        self.submit_msg(DbMsg::Share {
            client_sub,
            machine_id,
            share_hash,
            work_done,
            block_height,
        });
    }

    pub fn submit_telemetry_proofrate(&self, proofrate: BTreeMap<Uuid, BTreeMap<Arc<str>, u32>>) {
        self.submit_msg(DbMsg::TelemetryProofrate {
            machines: proofrate,
        });
    }

    pub fn submit_telemetry_hwinfo(
        &self,
        hwinfo: BTreeMap<Uuid, BTreeMap<Arc<str>, DeviceInfoWithSockets>>,
    ) {
        self.submit_msg(DbMsg::TelemetryHwinfo { machines: hwinfo });
    }

    pub fn submit_abort(&self, client_sub: Uuid, machine_id: Arc<str>, reason: AbortReason) {
        self.submit_msg(DbMsg::Abort {
            client_sub,
            machine_id,
            reason,
        });
    }

    pub fn submit_ip_address_details(
        &self,
        sub: Uuid,
        ip_address: IpAddr,
        decision: IpAddressCheckDecision,
        response: String,
    ) {
        self.submit_msg(DbMsg::IpAddressDetails {
            sub,
            ip_address,
            decision,
            response,
        });
    }

    pub async fn check_ip_address_details(
        &self,
        sub: Uuid,
        ip_address: IpAddr,
    ) -> Option<IpAddressCheckDecision> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if self
            .msgs
            .try_send(DbMsg::CheckIpAddress {
                sub,
                ip_address,
                response: tx,
            })
            .is_err()
        {
            counter!("nbx_miner_db_ip_address_check_error_total").increment(1);
            // Fail open: check the ip address again
            return None;
        }

        rx.await.ok()?
    }

    pub async fn is_blocklisted(&self, sub: Uuid) -> (bool, Option<String>) {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if self
            .msgs
            .try_send(DbMsg::CheckBlocklist { sub, response: tx })
            .is_err()
        {
            counter!("nbx_miner_db_blocklist_check_error_total").increment(1);
            // Fail open: allow connection if we can't check
            return (false, None);
        }

        rx.await.unwrap_or_else(|_| {
            error!("Database blocklist check channel closed");
            (false, None) // Fail open
        })
    }

    pub fn blocklist(&self, sub: Uuid, message: String) {
        self.submit_msg(DbMsg::Block { sub, message });
    }

    pub async fn is_allow_listed(&self, sub: Uuid) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if self
            .msgs
            .try_send(DbMsg::CheckAllowlist { sub, response: tx })
            .is_err()
        {
            counter!("nbx_miner_db_allowlist_check_error_total").increment(1);
            // Fail closed: don't assume that the sub is allow listed
            return false;
        }

        rx.await.unwrap_or_else(|_| {
            error!("Database allowlist check channel closed");
            false // Fail closed
        })
    }

    pub async fn get_all_blocklisted_subs(&self) -> Option<HashSet<Uuid>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if self
            .msgs
            .try_send(DbMsg::GetAllBlocklisted { response: tx })
            .is_err()
        {
            counter!("nbx_miner_db_get_all_blocklisted_error_total").increment(1);
            return None;
        }

        rx.await.unwrap_or_else(|_| {
            error!("Database get_all_blocklisted channel closed");
            counter!("nbx_miner_db_get_all_blocklisted_channel_error_total").increment(1);
            None
        })
    }

    pub async fn get_all_difficulty_bucket_assignments(&self) -> Option<HashMap<Uuid, i32>> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        if self
            .msgs
            .try_send(DbMsg::GetAllDifficultyBucketAssignments { response: tx })
            .is_err()
        {
            counter!("nbx_miner_db_get_all_difficulty_buckets_error_total").increment(1);
            return None;
        }

        rx.await.unwrap_or_else(|_| {
            error!("Database get_all_difficulty_buckets channel closed");
            counter!("nbx_miner_db_get_all_difficulty_buckets_channel_error_total").increment(1);
            None
        })
    }
}
