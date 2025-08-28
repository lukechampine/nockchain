use std::str::FromStr;
use std::sync::Mutex as SyncMutex;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{value_parser, Args};
use gdt_cpus::CoreType;
use kernels::miner::KERNEL;
use nockapp::kernel::form::SerfThread;
use nockapp::nockapp::driver::{IODriverFn, NockAppHandle, PokeResult};
use nockapp::nockapp::wire::Wire;
use nockapp::nockapp::NockAppError;
use nockapp::noun::slab::{slab_equality, NockJammer, NounSlab};
use nockapp::noun::{AtomExt, NounExt};
use nockapp::save::SaveableCheckpoint;
use nockapp::utils::NOCK_STACK_SIZE_TINY;
use nockapp::{Bytes, CrownError, Noun};
use nockchain_libp2p_io::tip5_util::tip5_hash_to_base58;
use nockvm::interpreter::NockCancelToken;
use nockvm::jets::hot::HotEntry;
use nockvm::noun::{Atom, D, NO, T, YES};
use nockvm_macros::tas;
use rand::Rng;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};
use zkvm_jetpack::form::PRIME;
use zkvm_jetpack::hand::structs::HoonList;
use zkvm_jetpack::noun::noun_ext::NounExt as OtherNounExt;

pub enum MiningWire {
    Mined,
    Candidate,
    SetPubKey,
    Enable,
}

impl MiningWire {
    pub fn verb(&self) -> &'static str {
        match self {
            MiningWire::Mined => "mined",
            MiningWire::SetPubKey => "setpubkey",
            MiningWire::Candidate => "candidate",
            MiningWire::Enable => "enable",
        }
    }
}

impl Wire for MiningWire {
    const VERSION: u64 = 1;
    const SOURCE: &'static str = "miner";

    fn to_wire(&self) -> nockapp::wire::WireRepr {
        let tags = vec![self.verb().into()];
        nockapp::wire::WireRepr::new(MiningWire::SOURCE, MiningWire::VERSION, tags)
    }
}

#[derive(Debug, Clone)]
pub struct MiningKeyConfig {
    pub share: u64,
    pub m: u64,
    pub keys: Vec<String>,
}

impl FromStr for MiningKeyConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expected format: "share,m:key1,key2,key3"
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid format. Expected 'share,m:key1,key2,key3'".to_string());
        }

        let share_m: Vec<&str> = parts[0].split(',').collect();
        if share_m.len() != 2 {
            return Err("Invalid share,m format".to_string());
        }

        let share = share_m[0].parse::<u64>().map_err(|e| e.to_string())?;
        let m = share_m[1].parse::<u64>().map_err(|e| e.to_string())?;
        let keys: Vec<String> = parts[1].split(',').map(String::from).collect();

        Ok(MiningKeyConfig { share, m, keys })
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct MiningConfig {
    #[arg(long, help = "Mine in-kernel", default_value = "false")]
    pub mine: bool,
    #[arg(
        long,
        help = "Pubkey to mine to (mutually exclusive with --mining-key-adv)"
    )]
    pub mining_pubkey: Option<String>,
    #[arg(
        long,
        help = "Advanced mining key configuration (mutually exclusive with --mining-pubkey). Format: share,m:key1,key2,key3",
        value_parser = value_parser!(MiningKeyConfig),
        num_args = 1..,
    )]
    pub mining_key_adv: Option<Vec<MiningKeyConfig>>,
    #[command(flatten)]
    pub server: nbx_miner::server::MiningConfig,
    #[command(flatten)]
    pub client: nbx_miner::client_base::ClientConfig,
}

impl MiningConfig {
    pub fn mining_key_config(&self) -> Option<Vec<MiningKeyConfig>> {
        if let Some(pubkey) = &self.mining_pubkey {
            Some(vec![MiningKeyConfig {
                share: 1,
                m: 1,
                keys: vec![pubkey.clone()],
            }])
        } else if let Some(mining_key_adv) = &self.mining_key_adv {
            Some(mining_key_adv.clone())
        } else {
            None
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mine && !(self.mining_pubkey.is_some() || self.mining_key_adv.is_some()) {
            return Err(
                "Cannot specify mine without either mining_pubkey or mining_key_adv".to_string(),
            );
        }

        if self.mining_pubkey.is_some() && self.mining_key_adv.is_some() {
            return Err(
                "Cannot specify both mining_pubkey and mining_key_adv at the same time".to_string(),
            );
        }

        Ok(())
    }
}

struct MiningData {
    pub block_header: NounSlab,
    pub version: NounSlab,
    pub target: NounSlab,
    pub pow_len: u64,
}

impl PartialEq for MiningData {
    fn eq(&self, other: &Self) -> bool {
        if self.pow_len != other.pow_len {
            return false;
        }
        if !slab_equality(&self.block_header, &other.block_header) {
            return false;
        }
        if !slab_equality(&self.version, &other.version) {
            return false;
        }
        if !slab_equality(&self.target, &other.target) {
            return false;
        }
        true
    }
}

pub fn create_mining_driver(
    cfg: MiningConfig,
    init_complete_tx: Option<tokio::sync::oneshot::Sender<()>>,
    server: TcpListener,
) -> IODriverFn {
    Box::new(move |handle| {
        Box::pin(async move {
            let Some(configs) = cfg.mining_key_config() else {
                enable_mining(&handle, false).await?;

                if let Some(tx) = init_complete_tx {
                    tx.send(()).map_err(|_| {
                        warn!("Could not send driver initialization for mining driver.");
                        NockAppError::OtherError
                    })?;
                }

                return Ok(());
            };
            if configs.len() == 1
                && configs[0].share == 1
                && configs[0].m == 1
                && configs[0].keys.len() == 1
            {
                set_mining_key(&handle, configs[0].keys[0].clone()).await?;
            } else {
                set_mining_key_advanced(&handle, configs).await?;
            }
            enable_mining(&handle, cfg.mine).await?;

            if let Some(tx) = init_complete_tx {
                tx.send(()).map_err(|_| {
                    warn!("Could not send driver initialization for mining driver.");
                    NockAppError::OtherError
                })?;
            }

            if !cfg.mine {
                return Ok(());
            }

            nbx_miner::server::mining_driver(cfg.server.clone(), server, handle).await
        })
    })
}

#[instrument(skip(handle, pubkey))]
async fn set_mining_key(
    handle: &NockAppHandle,
    pubkey: String,
) -> Result<PokeResult, NockAppError> {
    let mut set_mining_key_slab = NounSlab::new();
    let set_mining_key = Atom::from_value(&mut set_mining_key_slab, "set-mining-key")
        .expect("Failed to create set-mining-key atom");
    let pubkey_cord =
        Atom::from_value(&mut set_mining_key_slab, pubkey).expect("Failed to create pubkey atom");
    let set_mining_key_poke = T(
        &mut set_mining_key_slab,
        &[D(tas!(b"command")), set_mining_key.as_noun(), pubkey_cord.as_noun()],
    );
    set_mining_key_slab.set_root(set_mining_key_poke);

    handle
        .poke(MiningWire::SetPubKey.to_wire(), set_mining_key_slab)
        .await
}

async fn set_mining_key_advanced(
    handle: &NockAppHandle,
    configs: Vec<MiningKeyConfig>,
) -> Result<PokeResult, NockAppError> {
    let mut set_mining_key_slab = NounSlab::new();
    let set_mining_key_adv = Atom::from_value(&mut set_mining_key_slab, "set-mining-key-advanced")
        .expect("Failed to create set-mining-key-advanced atom");

    // Create the list of configs
    let mut configs_list = D(0);
    for config in configs {
        // Create the list of keys
        let mut keys_noun = D(0);
        for key in config.keys {
            let key_atom =
                Atom::from_value(&mut set_mining_key_slab, key).expect("Failed to create key atom");
            keys_noun = T(&mut set_mining_key_slab, &[key_atom.as_noun(), keys_noun]);
        }

        // Create the config tuple [share m keys]
        let config_tuple = T(
            &mut set_mining_key_slab,
            &[D(config.share), D(config.m), keys_noun],
        );

        configs_list = T(&mut set_mining_key_slab, &[config_tuple, configs_list]);
    }

    let set_mining_key_poke = T(
        &mut set_mining_key_slab,
        &[D(tas!(b"command")), set_mining_key_adv.as_noun(), configs_list],
    );
    set_mining_key_slab.set_root(set_mining_key_poke);

    handle
        .poke(MiningWire::SetPubKey.to_wire(), set_mining_key_slab)
        .await
}

//TODO add %set-mining-key-multisig poke
#[instrument(skip(handle))]
async fn enable_mining(handle: &NockAppHandle, enable: bool) -> Result<PokeResult, NockAppError> {
    let mut enable_mining_slab = NounSlab::new();
    let enable_mining = Atom::from_value(&mut enable_mining_slab, "enable-mining")
        .expect("Failed to create enable-mining atom");
    let enable_mining_poke = T(
        &mut enable_mining_slab,
        &[D(tas!(b"command")), enable_mining.as_noun(), if enable { YES } else { NO }],
    );
    enable_mining_slab.set_root(enable_mining_poke);
    handle
        .poke(MiningWire::Enable.to_wire(), enable_mining_slab)
        .await
}
