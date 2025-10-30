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
use nockchain_types::tx_engine::common::{Hash, SchnorrPubkey};
use nockvm::interpreter::NockCancelToken;
use nockvm::jets::hot::HotEntry;
use nockvm::noun::{Atom, D, NO, T, YES};
use nockvm_macros::tas;
use rand::Rng;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};
use zkvm_jetpack::form::belt::PRIME;
use zkvm_jetpack::form::noun_ext::NounMathExt;
use zkvm_jetpack::form::structs::HoonList;

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

#[derive(Debug, Clone)]
pub struct MiningPkhConfig {
    pub share: u64,
    pub pkh: String,
}

impl FromStr for MiningPkhConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Expected format: "share,pkh"
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Err("Invalid share,pkh format".to_string());
        }

        let share = parts[0].parse::<u64>().map_err(|e| e.to_string())?;
        let pkh = parts[1].parse::<String>().map_err(|e| e.to_string())?;

        Ok(MiningPkhConfig { share, pkh })
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct MiningConfig {
    #[arg(long, help = "Mine in-kernel", default_value = "false")]
    pub mine: bool,
    #[arg(
        long,
        help = "Pubkey hash to mine to (mutually exclusive with --mining-pkh-adv)"
    )]
    pub mining_pkh: Option<String>,
    #[arg(
        long,
        help = "Advanced mining pubkey hash configuration (mutually exclusive with --mining-pkh). Format: share,pkh",
        value_parser = value_parser!(MiningPkhConfig),
        num_args = 1..,
    )]
    pub mining_pkh_adv: Option<Vec<MiningPkhConfig>>,
    #[command(flatten)]
    pub server: nbx_miner::server::MiningConfig,
}

impl MiningConfig {
    pub fn mining_pkh_config(&self) -> Option<Vec<MiningPkhConfig>> {
        if let Some(pkh) = &self.mining_pkh {
            Some(vec![MiningPkhConfig {
                share: 1,
                pkh: pkh.clone(),
            }])
        } else if let Some(mining_pkh_adv) = &self.mining_pkh_adv {
            Some(mining_pkh_adv.clone())
        } else {
            None
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mine && !(self.mining_pkh.is_some() || self.mining_pkh_adv.is_some()) {
            return Err(
                "Cannot specify mine without either mining_pkh or mining_pkh_adv".to_string(),
            );
        }

        if self.mining_pkh.is_some() && self.mining_pkh_adv.is_some() {
            return Err(
                "Cannot specify both mining_pkh and mining_pkh_adv at the same time".to_string(),
            );
        }

        if let Some(pkh) = &self.mining_pkh {
            Hash::from_base58(pkh).map_err(|err| format!("Invalid mining_pkh: {err}"))?;
        }

        if let Some(pkh_configs) = &self.mining_pkh_adv {
            for config in pkh_configs {
                Hash::from_base58(&config.pkh).map_err(|err| {
                    format!("Invalid mining_pkh_adv entry '{}': {err}", config.pkh)
                })?;
            }
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
            // set up empty config for v0 keys (TODO remove when taking out pubkey infra)
            let configs = Vec::<MiningKeyConfig>::new();

            let mining_pkh_config = if let Some(pkh) = &cfg.mining_pkh {
                Some(vec![MiningPkhConfig {
                    share: 1,
                    pkh: pkh.clone(),
                }])
            } else if let Some(mining_pkh_adv) = &cfg.mining_pkh_adv {
                Some(mining_pkh_adv.clone())
            } else {
                None
            };

            let Some(pkh_configs) = mining_pkh_config else {
                enable_mining(&handle, false).await?;

                if let Some(tx) = init_complete_tx {
                    tx.send(()).map_err(|_| {
                        NockAppError::OtherError(String::from(
                            "Could not send driver initialization for mining driver.",
                        ))
                    })?;
                }

                return Ok(());
            };
            set_mining_key_advanced(&handle, configs, pkh_configs).await?;
            enable_mining(&handle, cfg.mine).await?;

            if let Some(tx) = init_complete_tx {
                tx.send(()).map_err(|_| {
                    NockAppError::OtherError(String::from(
                        "Could not send driver initialization for mining driver.",
                    ))
                })?;
            }

            if !cfg.mine {
                return Ok(());
            }

            nbx_miner::server::mining_driver(cfg.server.clone(), server, handle).await
        })
    })
}

async fn set_mining_key_advanced(
    handle: &NockAppHandle,
    configs: Vec<MiningKeyConfig>,
    pkh_configs: Vec<MiningPkhConfig>,
) -> Result<PokeResult, NockAppError> {
    let mut set_mining_key_slab = NounSlab::new();
    let set_mining_key_adv = Atom::from_value(&mut set_mining_key_slab, "set-mining-key-advanced")
        .expect("Failed to create set-mining-key-advanced atom");

    // Create the list of v0 (pubkey) configs (TODO remove when taking out pubkey infra)
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

    // Create the list of v1 (pubkey hash) configs
    let mut pkh_configs_list = D(0);
    for config in pkh_configs {
        let pkh_noun = Atom::from_value(&mut set_mining_key_slab, config.pkh)
            .expect("Failed to create key atom")
            .as_noun();

        // Create the config tuple [share pkh]
        let config_tuple = T(&mut set_mining_key_slab, &[D(config.share), pkh_noun]);

        pkh_configs_list = T(&mut set_mining_key_slab, &[config_tuple, pkh_configs_list]);
    }

    let set_mining_key_poke = T(
        &mut set_mining_key_slab,
        &[
            D(tas!(b"command")),
            set_mining_key_adv.as_noun(),
            configs_list,
            pkh_configs_list,
        ],
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
