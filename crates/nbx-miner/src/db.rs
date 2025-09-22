use std::collections::BTreeMap;
use std::sync::Arc;

use ibig::UBig;
use metrics::{counter, gauge};
use nbx_jetpack::log::error;
use nockchain_libp2p_io::tip5_util::ubig_to_base58;
use sqlx::any::{install_default_drivers, AnyPoolOptions};
use sqlx::AnyPool;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::device::DeviceInfo;
use crate::server::AbortReason;

enum DbMsg {
    Share {
        client_sub: Arc<str>,
        machine_id: Arc<str>,
        share_hash: String,
        work_done: u64,
    },
    TelemetryProofrate {
        machines: BTreeMap<Arc<str>, BTreeMap<Arc<str>, u32>>,
    },
    TelemetryHwinfo {
        machines: BTreeMap<Arc<str>, BTreeMap<Arc<str>, DeviceInfo>>,
    },
    Abort {
        client_sub: Arc<str>,
        machine_id: Arc<str>,
        reason: AbortReason,
    },
}

pub(crate) struct Database {
    pool: AnyPool,
    msgs: Receiver<DbMsg>,
    src_name: Arc<str>,
}

impl Database {
    pub async fn new(db_url: &str, src_name: Arc<str>) -> sqlx::Result<(Self, DatabaseHandle)> {
        install_default_drivers();
        let pool = AnyPoolOptions::new().connect(db_url).await?;

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
                DbMsg::Share { client_sub, machine_id, share_hash, work_done } => {
                    sqlx::query("INSERT INTO \"shares\" (src, sub, machine_id, share_hash, accumulated_work) VALUES ( $1, $2, $3, $4, $5 )")
                        .bind(&*src_name)
                        .bind(&*client_sub)
                        .bind(&*machine_id)
                        .bind(share_hash)
                        .bind(i64::try_from(work_done).unwrap_or(i64::MAX))
                        .execute(&pool)
                        .await
                }
                DbMsg::TelemetryProofrate {
                    machines
                } => {
                    let query = "INSERT INTO \"proofrate\" (src, sub, machine_id, proof_rate)".to_string();
                    let mut values = vec![];
                    let mut binding = 2;
                    for (_, m) in &machines {
                        let sub_binding = binding;
                        binding += 1;
                        for _ in m {
                            values.push(format!("( $1, ${sub_binding}, ${}, ${} )", binding, binding + 1));
                            binding += 2;
                        }
                    }
                    let query = format!("{query} VALUES {}", values.join(", "));
                    let mut q = sqlx::query(&query)
                        .bind(&*src_name);
                    for (sub, m) in machines {
                        q = q.bind(sub.to_string());
                        for (mid, pr) in m {
                            q = q.bind(mid.to_string()).bind(pr as i64);
                        }
                    }
                    q.execute(&pool).await
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
                        q = q.bind(sub.to_string());
                        for (mid, dev) in m {
                            q = q.bind(mid.to_string()).bind(dev.is_proxy).bind(serde_json::to_string(&dev).unwrap());
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
                        .bind(&*client_sub)
                        .bind(&*machine_id)
                        .bind(reason.to_string())
                        .execute(&pool)
                        .await
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
        client_sub: Arc<str>,
        machine_id: Arc<str>,
        share_hash: UBig,
        work_done: u64,
    ) {
        let share_hash = ubig_to_base58(share_hash);
        self.submit_msg(DbMsg::Share {
            client_sub,
            machine_id,
            share_hash,
            work_done,
        });
    }

    pub fn submit_telemetry_proofrate(
        &self,
        proofrate: BTreeMap<Arc<str>, BTreeMap<Arc<str>, u32>>,
    ) {
        self.submit_msg(DbMsg::TelemetryProofrate {
            machines: proofrate,
        });
    }

    pub fn submit_telemetry_hwinfo(
        &self,
        hwinfo: BTreeMap<Arc<str>, BTreeMap<Arc<str>, DeviceInfo>>,
    ) {
        self.submit_msg(DbMsg::TelemetryHwinfo { machines: hwinfo });
    }

    pub fn submit_abort(&self, client_sub: Arc<str>, machine_id: Arc<str>, reason: AbortReason) {
        self.submit_msg(DbMsg::Abort {
            client_sub,
            machine_id,
            reason,
        });
    }
}
