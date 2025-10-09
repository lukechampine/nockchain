use std::collections::BTreeMap;
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
                        .await
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

                    sqlx::query(query)
                        .bind(&subs)
                        .bind(&machine_ids)
                        .bind(&proofrates)
                        .execute(&pool)
                        .await
                }
                DbMsg::TelemetryHwinfo {
                    machines
                } => {
                    let query = "INSERT INTO \"machines\" (src, sub, machine_id, is_proxy, hardware)".to_string();
                    let mut values = vec![];
                    let mut binding = 2;
                    for (_, m) in &machines {
                        let sub_binding = binding;
                        binding += 1;
                        for _ in m {
                            values.push(format!("( $1, ${sub_binding}, ${}, ${}, ${} )", binding, binding + 1, binding + 2));
                            binding += 3;
                        }
                    }
                    let query = format!("{query} VALUES {}", values.join(", "));
                    let mut q = sqlx::query(&query)
                        .bind(&*src_name);
                    for (sub, m) in machines {
                        q = q.bind(sub);
                        for (mid, dev) in m {
                            q = q.bind(mid.to_string()).bind(dev.device.is_proxy).bind(Json(dev));
                        }
                    }
                    q.execute(&pool).await
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
                        .await
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

                    create_result
                }
            };

            if let Err(e) = r {
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

    pub fn partition_maintenance(&self) {
        self.submit_msg(DbMsg::PartitionMaintenance);
    }
}
