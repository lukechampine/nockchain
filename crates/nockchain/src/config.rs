use std::path::PathBuf;
use std::time::Duration;

use clap::{arg, command, value_parser, ArgAction, Parser};
use nockchain_types::tx_engine::common::{Hash, SchnorrPubkey};

use crate::mining::MiningConfig;

// TODO: command-line/configure
/** Path to read current node's identity from */
pub const IDENTITY_PATH: &str = ".nockchain_identity";

/** Path to read current node's peer ID from */
pub const PEER_ID_EXTENSION: &str = ".peer_id";

// TODO: command-line/configure
/** Extension for peer ID files */
pub const PEER_ID_FILE_EXTENSION: &str = "peerid";

// Libp2p multiaddrs don't support const construction, so we have to put strings literals and parse them at startup
/** Backbone nodes for our testnet */
pub const TESTNET_BACKBONE_NODES: &[&str] = &[];

// Libp2p multiaddrs don't support const construction, so we have to put strings literals and parse them at startup
// TODO: feature flag testnet/realnet
/** Backbone nodes for our realnet */
#[allow(dead_code)]
pub const REALNET_BACKBONE_NODES: &[&str] = &["/dnsaddr/nockchain-backbone.zorp.io"];

/** How often we should affirmatively ask other nodes for their heaviest chain */
pub const CHAIN_INTERVAL: Duration = Duration::from_secs(20);

/// The height of the bitcoin block that we want to sync our genesis block to
/// Currently, this is the height of an existing block for testing. It will be
/// switched to a future block for launch.
pub const GENESIS_HEIGHT: u64 = 897767;

/// Command line arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "nockchain")]
pub struct NockchainCli {
    #[command(flatten)]
    pub nockapp_cli: nockapp::kernel::boot::Cli,
    #[command(flatten)]
    pub miner: MiningConfig,
    #[arg(long, help = "Whether to run as fakenet", default_value_t = false)]
    pub fakenet: bool,
    #[arg(long, short, help = "Initial peer", action = ArgAction::Append)]
    pub peer: Vec<String>,
    #[arg(long, short, help = "Force peer", action = ArgAction::Append)]
    pub force_peer: Vec<String>,
    #[arg(long, help = "Allowed peer IDs file")]
    pub allowed_peers_path: Option<String>,
    #[arg(long, help = "Don't dial default peers")]
    pub no_default_peers: bool,
    #[arg(long, help = "Bind address", action = ArgAction::Append)]
    pub bind: Option<Vec<String>>,
    #[arg(
        long,
        help = "Don't generate a new peer ID, keep the existing one",
        default_value = "false"
    )]
    pub no_new_peer_id: bool,
    #[arg(long, help = "Maximum established incoming connections")]
    pub max_established_incoming: Option<u32>,
    #[arg(long, help = "Maximum established outgoing connections")]
    pub max_established_outgoing: Option<u32>,
    #[arg(long, help = "Maximum pending incoming connections")]
    pub max_pending_incoming: Option<u32>,
    #[arg(long, help = "Maximum pending outgoing connections")]
    pub max_pending_outgoing: Option<u32>,
    #[arg(long, help = "Maximum established connections")]
    pub max_established: Option<u32>,
    #[arg(long, help = "Maximum established connections per peer")]
    pub max_established_per_peer: Option<u32>,
    #[arg(
        long,
        help = "Prune <N> inbound connections when a peer is denied due to connection limits. (Use on boot nodes only.)"
    )]
    pub prune_inbound: Option<usize>,
    #[arg(long, help = "Maximum system memory percentage for connection limits")]
    pub max_system_memory_fraction: Option<f64>,
    #[arg(long, help = "Maximum process memory for connection limits (bytes)")]
    pub max_system_memory_bytes: Option<usize>,
    #[arg(
        long,
        help = "Size of Proof of Work puzzle for mining on fakenet. Mainnet uses 64. Must be a power of 2. Defaults to 2. Ignored on mainnet.",
        default_value = "2",
        requires = "fakenet"
    )]
    pub fakenet_pow_len: u64,
    #[arg(
        long,
        help = "log target difficulty for mining on fakenet. Defaults to 1 (so 2^1 attempts on average find a block). Ignored on mainnet.",
        default_value = "1",
        requires = "fakenet"
    )]
    pub fakenet_log_difficulty: u64,
    #[arg(
        long,
        help = "Minimum timelock for coinbase transactions on fakenet. Defaults to 100 blocks. Ignored on mainnet.",
        default_value = "100",
        requires = "fakenet"
    )]
    pub fakenet_coinbase_timelock_min: Option<u64>,
    #[arg(
        long,
        help = "Override the v1-phase activation height when running on fakenet. Requires --fakenet.",
        default_value = "1",
        requires = "fakenet"
    )]
    pub fakenet_v1_phase: Option<u64>,
    #[arg(long, help = "Path to fake genesis block jam file")]
    pub fakenet_genesis_jam_path: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:9005")]
    pub prometheus_bind: String,
    #[arg(long, value_parser = clap::value_parser!(std::net::SocketAddr), default_value = "127.0.0.1:5555")]
    pub bind_public_grpc_addr: std::net::SocketAddr,
    #[arg(long, default_value = "5555")]
    pub bind_private_grpc_port: u16,
    #[arg(long, default_value = "false")]
    pub fast_sync: bool,
}

impl NockchainCli {
    pub fn validate(&self) -> Result<(), String> {
        self.miner.validate()
    }
}

#[cfg(test)]
mod tests {
    use nockapp::kernel::boot::default_boot_cli;

    use super::*;

    const VALID_V0_PUBKEY: &str = "2cPnE4Z9RevhTv9is9Hmc1amFubEFbUxzCV2Fxb9GxevJstV5VG92oYt6Sai3d3NjLFcsuVXSLx9hikMbD1agv9M267TVw3hV9MCpMfEnGo5LYtjJ7jPyHg8SERPjJRCWTgZ";
    const VALID_MINING_PKH: &str = "9yPePjfWAdUnzaQKyxcRXKRa5PpUzKKEwtpECBZsUYt9Jd7egSDEWoV";

    fn base_cli() -> NockchainCli {
        NockchainCli {
            nockapp_cli: default_boot_cli(false),
            miner: MiningConfig {
                mine: false,
                mining_pkh: None,
                mining_pkh_adv: None,
                server: Default::default(),
            },
            fakenet: false,
            peer: Vec::new(),
            force_peer: Vec::new(),
            allowed_peers_path: None,
            no_default_peers: false,
            bind: None,
            no_new_peer_id: false,
            max_established_incoming: None,
            max_established_outgoing: None,
            max_pending_incoming: None,
            max_pending_outgoing: None,
            max_established: None,
            max_established_per_peer: None,
            prune_inbound: None,
            max_system_memory_fraction: None,
            max_system_memory_bytes: None,
            fakenet_pow_len: 2,
            fakenet_log_difficulty: 1,
            fakenet_v1_phase: None,
            fakenet_genesis_jam_path: None,
            fakenet_coinbase_timelock_min: None,
            bind_public_grpc_addr: "127.0.0.1:5555".parse().unwrap(),
            bind_private_grpc_port: 5555,
            fast_sync: false,
        }
    }

    #[test]
    fn validate_accepts_valid_advanced_configs() {
        let mut cli = base_cli();
        cli.mining_pkh_adv = Some(vec![MiningPkhConfig {
            share: 1,
            pkh: VALID_MINING_PKH.to_string(),
        }]);

        assert!(cli.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_mining_pkh_adv_entry() {
        // We specifically want to catch if users mix up v0 and v1 addresses, because they are both base58-encoded.
        // Using a base58-encoded pubkey ensures the input is base58 but not a valid hash.
        let mut cli = base_cli();
        cli.mining_pkh_adv = Some(vec![MiningPkhConfig {
            share: 1,
            pkh: VALID_V0_PUBKEY.to_string(),
        }]);

        let err = cli.validate().expect_err("expected invalid pkh adv");
        assert!(err.contains("Invalid mining_pkh_adv entry"));
    }
}
