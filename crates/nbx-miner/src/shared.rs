use std::collections::BTreeMap;
use std::sync::Arc;

use nockapp::noun::slab::{slab_equality, NounSlab};
use nockapp::wire::Wire;

#[derive(Debug)]
pub struct MiningData {
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

#[derive(Clone, Debug)]
pub struct MiningResult {
    pub miner_id: usize,
    pub attempt_seconds: f32,
    pub is_block: bool,
    pub poke: Option<NounSlab>,
    pub effect: Option<NounSlab>,
}

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
