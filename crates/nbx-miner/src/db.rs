use std::sync::Arc;

use clap::Args;
use sqlx::PgPool;

#[derive(Args, Clone, Debug, Default)]
pub struct DbConfig {
    pub database_address: String,
    pub database_port: usize,
    pub database_name: String,
}

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(cfg: DbConfig) -> Option<Self> {
        None
    }
}

pub struct DatabaseHandle {

}

impl DatabaseHandle {
    pub fn submit_share(&self, client_sub: Arc<str>, machine_id: Arc<str>, share_hash: &[u8], work_done: u64) {

    }
}
