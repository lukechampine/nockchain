use std::collections::BTreeMap;
use std::sync::Mutex;

use either::Either::{self, *};
use nockvm::interpreter::Context;
use nockvm::jets::hot::{HotEntry, K_138};
use nockvm::jets::util::slot;
use nockvm::jets::{JetErr, Result};
use nockvm::noun::*;

mod eight;
mod five;
mod four;
mod hoon;
mod one;
mod prover;
mod prover_compute;
mod prover_memory;
mod seven;
mod six;
mod three;
mod two;
mod utils;
mod zoon;

pub mod codewords;
pub mod deep;
#[cfg(feature = "gpu")]
pub mod gpu;
mod hash;
pub mod substitute;

pub mod engine;
pub mod instruments;

pub use eight::compute_table_polys;
use eight::*;
use five::*;
use hoon::*;
pub use one::snag_as_poly_mary;
use one::*;
use prover::*;
use prover_compute::build as compute_build;
use prover_memory::{build_v0_v1 as memory_build_v0_v1, build_v2 as memory_build_v2, *};
use six::*;
use three::*;
use two::*;
pub use two::{bpoly_to_fpoly, mp_substitute_ultra_impl, new_fpoly};
use zoon::*;

#[cfg(not(feature = "codefuscate"))]
#[macro_export]
macro_rules! codefuscate {
    ($($tt:tt)*) => { $($tt)* }
}

#[cfg(feature = "codefuscate")]
pub use goldberg::goldberg_stmts as codefuscate;

pub mod log {
    pub use tracing::log::*;
}

macro_rules! jam_err {
    ($name:ident) => {{
        let jam_dir = concat!("./jams/", stringify!($name));
        JetErr::PuntJam(jam_dir)
    }};
}

macro_rules! jet_option {
    ($name:ident => : $b:expr) => { $b };
    // Run only only once, otherwise crash
    ($name:ident => 'run_once $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                static RUN: AtomicBool = AtomicBool::new(false);

                if !RUN.fetch_or(true, Ordering::Relaxed) {
                    $b
                } else {
                    jet_err()
                }
            }
        }
    };
    // Write jam files on crashes
    ($name:ident => 'jam_errs $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                if ret.is_err() {
                    Err(jam_err!($name))
                } else {
                    ret
                }
            }
        }
    };
    // Bypass crashes (reinterpret them)
    ($name:ident => 'punt_errs $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                if ret.is_err() {
                    Err(JetErr::Punt)
                } else {
                    ret
                }
            }
        }
    };
    // Jam invokations
    ($name:ident => 'jam $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let _ret = $b;
                Err(jam_err!($name))
            }
        }
    };
    // Create jam directory
    ($name:ident => 'create_jam_dir $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                match ret {
                    Err(JetErr::PuntJam(d)) => {
                        let _ = std::fs::create_dir_all(d);
                        Err(JetErr::PuntJam(d))
                    }
                    v => v
                }
            }
        }
    };
    // Log invokations
    ($name:ident => 'log $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                println!("Jet invoked: {}", stringify!($name));
                $b
            }
        }
    };
}

/// Extracts sample and calls the jet implementation.
///
/// This is so that we can have callable implementations for composing jets.
macro_rules! sam_jet {
    ($name:ident => $imp:ident 'raw $($l:lifetime)*$(,)?) => {
        #[tracing::instrument(skip_all)]
        pub fn $name(context: &mut Context, subject: Noun) -> Result {
            jet_option!($imp => $($l)*: {
                $imp(context, subject)
            })
        }
    };
    ($name:ident => $imp:ident $($l:lifetime)*$(,)?) => {
        #[tracing::instrument(skip_all)]
        pub fn $name(context: &mut Context, subject: Noun) -> Result {
            jet_option!($imp => $($l)*: {
                let sam = slot(subject, 6)?;
                $imp(&mut context.stack, sam)
            })
        }
    };
    ($name:ident => $imp:ident $($l:lifetime)*, $($rest:tt)*) => {
        sam_jet!($name => $imp $($l)*);
        sam_jet!($($rest)*);
    };
}

sam_jet! {
    hash_10_jet => hash_10_sam,
    //hash_belts_list_jet => hash_belts_list,
    hash_noun_varlen_jet => hash_noun_varlen,
    //hash_pairs_jet => hash_pairs,
    hash_varlen_jet => hash_varlen_sam,
    hash_hashable_jet => hash_hashable,// 'jam 'create_jam_dir,
    hash_ten_cell_jet => hash_ten_cell,
    leaf_sequence_jet => leaf_sequence,
    mp_substitute_ultra_jet => mp_substitute_ultra, // 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    compute_composition_poly_jet => compute_composition_poly, // 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    compute_deep_jet => compute_deep,// 'jam 'create_jam_dir,
    fp_fft_jet => fp_fft_sam,
    fp_ifft_jet => fp_ifft_sam,
    fp_ntt_jet => fp_ntt_sam,
    do_init_mary_jet => do_init_mary,// 'jam 'create_jam_dir,
    // bpdiv_jet => bpdiv 'jam 'create_jam_dir,
    zero_extend_jet => zero_extend 'raw,// 'jam 'create_jam_dir,
    weld_step_jet => weld_step 'raw,// 'jam 'create_jam_dir,
    bp_build_merk_heap_jet => bp_build_merk_heap, //'jam 'create_jam_dir,
    build_merk_heap_jet => build_merk_heap, //'jam 'create_jam_dir,
    bpeval_lift_jet => bpeval_lift_sam,
    bstack_push_all_jet => bstack_push_all 'raw,
    fstack_push_all_jet => fstack_push_all 'raw,
    pstack_push_all_jet => pstack_push_all 'raw,
    bstack_push_jet => bstack_push 'raw,
    fstack_push_jet => fstack_push 'raw,
    pstack_push_jet => pstack_push 'raw,
    rna_bfta_jet => rna_bfta_sam,
    memory_build_v0_v1_jet => memory_build_v0_v1,
    memory_build_v2_jet => memory_build_v2,
    compute_build_jet => compute_build,
    precompute_ntts_jet => precompute_ntts,
    turn_coseword_jet => turn_coseword,
    pad_jet => pad,
    prove_fri_door_jet => prove_fri_door 'raw 'jam 'create_jam_dir,
    prove_commit_jet => prove_commit 'raw,// 'jam 'create_jam_dir,
    //absorb_proof_objects_jet => absorb_proof_objects //'jam 'create_jam_dir,
    zby_key_jet => zby_key 'raw,
    tog_belts_jet => tog_belts 'raw,
    tog_felts_jet => tog_felts 'raw,
    bp_decompose_jet => bp_decompose,
    bpeval_jet => bpeval,
    fp_decompose_jet => fp_decompose,
    fpeval_jet => fpeval,
    lift_to_fpoly_jet => lift_to_fpoly,
    binv_jet => binv_sam,
    compute_lde_jet => compute_lde_sam,
    compute_codeword_commitments_jet => compute_codeword_commitments_sam,
    interpolate_table_jet => interpolate_table_sam,
    bp_intercosate_jet => bp_intercosate_sam,
    bp_shift_by_unity_jet => bp_shift_by_unity_sam,
    sort_jet => list_sort 'raw,

    table_heights_jet => table_heights,
    proof_stream_push_jet => proof_stream_push 'raw,
}

const ENC_KEY: u32 = obfstr::random!(u32, "key");

const fn jetname_xor<const N: usize>(b: &[u8; N]) -> [u8; N] {
    let k = obfstr::bytes::keystream::<N>(ENC_KEY);
    let mut b2 = *b;
    jetname_xor_varlen(&mut b2, &k);
    b2
}

const fn jetname_xor_varlen(b: &mut [u8], k: &[u8]) {
    assert!(b.len() <= k.len());
    let mut i = 0usize;
    while i < b.len() {
        b[i] = b[i] ^ k[i];
        i += 1;
    }
}

macro_rules! jet_str {
    ($v:expr) => {
        Left(&jetname_xor($v))
    };
}

type JetPathEntry = Either<&'static [u8], (u64, u64)>;
type JetPath = [JetPathEntry];

fn heap_xor(v: &'static JetPath) -> &'static JetPath {
    static CACHE: Mutex<BTreeMap<(usize, usize), &'static JetPath>> = Mutex::new(BTreeMap::new());
    let mut cache = CACHE.lock().unwrap();
    *cache
        .entry((v as *const JetPath as *const () as usize, v.len()))
        .or_insert_with(|| {
            let cloned: Vec<JetPathEntry> = v
                .iter()
                .map(|e| match e {
                    Either::Left(bytes) => {
                        codefuscate! {
                            let mut buf = bytes.to_vec();
                            jetname_xor_varlen(&mut buf, &obfstr::bytes::keystream::<32>(ENC_KEY));
                            let leaked = Box::leak(buf.into_boxed_slice());
                            Either::Left(&*leaked)
                        }
                    }
                    Either::Right(pair) => Either::Right(*pair),
                })
                .collect();
            Box::leak(cloned.into_boxed_slice())
        })
}

pub const NBX_ONE_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"range"),
        ],
        1,
        range_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ave"),
            jet_str!(b"zero-extend"),
        ],
        1,
        zero_extend_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ave"),
            jet_str!(b"weld-step"),
        ],
        1,
        weld_step_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"do-init-mary"),
        ],
        1,
        do_init_mary_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"bp-decompose"),
        ],
        1,
        bp_decompose_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"bpeval"),
        ],
        1,
        bpeval_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"binv"),
        ],
        1,
        binv_jet,
    ),
];

pub const NBX_TWO_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"mp-substitute-ultra"),
        ],
        1,
        mp_substitute_ultra_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"fp-ntt"),
        ],
        1,
        fp_ntt_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"fp-fft"),
        ],
        1,
        fp_fft_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"fp-ifft"),
        ],
        1,
        fp_ifft_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"bpeval-lift"),
        ],
        1,
        bpeval_lift_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"turn-coseword"),
        ],
        1,
        turn_coseword_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"fp-decompose"),
        ],
        1,
        fp_decompose_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"fpeval"),
        ],
        1,
        fpeval_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"lift-to-fpoly"),
        ],
        1,
        lift_to_fpoly_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"interpolate-table"),
        ],
        1,
        interpolate_table_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"bp-intercosate"),
        ],
        1,
        bp_intercosate_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"bp-shift-by-unity"),
        ],
        1,
        bp_shift_by_unity_jet,
    ),
];

pub const NBX_THREE_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"shape"),
            jet_str!(b"leaf-sequence"),
        ],
        1,
        leaf_sequence_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"hash-noun-varlen"),
        ],
        1,
        hash_noun_varlen_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"hash-varlen"),
        ],
        1,
        hash_varlen_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"hash-10"),
        ],
        1,
        hash_10_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"hash-ten-cell"),
        ],
        1,
        hash_ten_cell_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"hash-hashable"),
        ],
        1,
        hash_hashable_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"tog"),
            jet_str!(b"belts"),
        ],
        1,
        tog_belts_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"tip5-lib"),
            jet_str!(b"tog"),
            jet_str!(b"felts"),
        ],
        1,
        tog_felts_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"merkle"),
            jet_str!(b"bp-build-merk-heap"),
        ],
        1,
        bp_build_merk_heap_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"merkle"),
            jet_str!(b"build-merk-heap"),
        ],
        1,
        build_merk_heap_jet,
    ),
];

// TODO: figure out how to build a core for `tog`, and enable this.
pub const NBX_FOUR_JETS: &[HotEntry] = &[/*(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"tri"),
        jet_str!(b"qua"),
        jet_str!(b"pen"),
        jet_str!(b"zeke"),
        jet_str!(b"ext-field"),
        jet_str!(b"misc-lib"),
        jet_str!(b"proof-lib"),
        jet_str!(b"absorb-proof-objects"),
    ],
    1,
    absorb_proof_objects_jet,
)*/];

pub const NBX_FIVE_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"constraint-util"),
            jet_str!(b"pstack"),
            jet_str!(b"push-all"),
        ],
        1,
        pstack_push_all_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"proof-stream"),
            jet_str!(b"push"),
        ],
        1,
        proof_stream_push_jet,
    ),
];

pub const NBX_SIX_JETS: &[HotEntry] = &[
    /*(
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"fri-door"),
            jet_str!(b"prove"),
        ],
        1,
        prove_fri_door_jet,
    ),*/
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"fri-door"),
            jet_str!(b"prove-commit"),
        ],
        1,
        prove_commit_jet,
    ),
];

pub const NBX_EIGHT_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"table-lib"),
            jet_str!(b"stark-core"),
            jet_str!(b"compute-composition-poly"),
        ],
        1,
        compute_composition_poly_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"table-lib"),
            jet_str!(b"stark-core"),
            jet_str!(b"compute-deep"),
        ],
        1,
        compute_deep_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"table-lib"),
            jet_str!(b"stark-core"),
            jet_str!(b"precompute-ntts"),
        ],
        1,
        precompute_ntts_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"table-lib"),
            jet_str!(b"stark-core"),
            jet_str!(b"compute-lde"),
        ],
        1,
        compute_lde_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"zeke"),
            jet_str!(b"ext-field"),
            jet_str!(b"misc-lib"),
            jet_str!(b"proof-lib"),
            jet_str!(b"utils"),
            jet_str!(b"fri"),
            jet_str!(b"table-lib"),
            jet_str!(b"stark-core"),
            jet_str!(b"compute-codeword-commitments"),
        ],
        1,
        compute_codeword_commitments_jet,
    ),
];

pub const NBX_MEMORY_V0_V1_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v0-v1"),
            jet_str!(b"rna-bfta"),
        ],
        1,
        rna_bfta_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v0-v1"),
            jet_str!(b"funcs"),
            jet_str!(b"build"),
        ],
        1,
        memory_build_v0_v1_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v0-v1"),
            jet_str!(b"funcs"),
            jet_str!(b"pad"),
        ],
        1,
        pad_jet,
    ),
];

pub const NBX_MEMORY_V2_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v2"),
            jet_str!(b"rna-bfta"),
        ],
        1,
        rna_bfta_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v2"),
            jet_str!(b"funcs"),
            jet_str!(b"build"),
        ],
        1,
        memory_build_v2_jet,
    ),
    (
        &[
            K_138,
            jet_str!(b"one"),
            jet_str!(b"two"),
            jet_str!(b"tri"),
            jet_str!(b"qua"),
            jet_str!(b"pen"),
            jet_str!(b"memory-table-v2"),
            jet_str!(b"funcs"),
            jet_str!(b"pad"),
        ],
        1,
        pad_jet,
    ),
];

pub const NBX_COMPUTE_V0_V1_JETS: &[HotEntry] = &[(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"tri"),
        jet_str!(b"qua"),
        jet_str!(b"pen"),
        jet_str!(b"compute-table-v0-v1"),
        jet_str!(b"funcs"),
        jet_str!(b"build"),
    ],
    1,
    compute_build_jet,
)];

pub const NBX_COMPUTE_V2_JETS: &[HotEntry] = &[(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"tri"),
        jet_str!(b"qua"),
        jet_str!(b"pen"),
        jet_str!(b"compute-table-v2"),
        jet_str!(b"funcs"),
        jet_str!(b"build"),
    ],
    1,
    compute_build_jet,
)];

pub const NBX_ZOON_JETS: &[HotEntry] = &[(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"tri"),
        jet_str!(b"qua"),
        jet_str!(b"pen"),
        jet_str!(b"zoon"),
        jet_str!(b"z-by"),
        jet_str!(b"key"),
    ],
    1,
    zby_key_jet,
)];

#[rustfmt::skip]
pub const NBX_HOON_JETS: &[HotEntry] = &[(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"sort"),
    ],
    1,
    sort_jet,
)];

pub const NBX_PROVER_JETS: &[HotEntry] = &[(
    &[
        K_138,
        jet_str!(b"one"),
        jet_str!(b"two"),
        jet_str!(b"tri"),
        jet_str!(b"qua"),
        jet_str!(b"pen"),
        jet_str!(b"zeke"),
        jet_str!(b"ext-field"),
        jet_str!(b"misc-lib"),
        jet_str!(b"proof-lib"),
        jet_str!(b"utils"),
        jet_str!(b"fri"),
        jet_str!(b"table-lib"),
        jet_str!(b"stark-core"),
        jet_str!(b"fock-core"),
        jet_str!(b"pow"),
        jet_str!(b"stark-engine"),
        jet_str!(b"stark-prover"),
        jet_str!(b"prove-door"),
        jet_str!(b"table-heights"),
    ],
    1,
    table_heights_jet,
)];

#[rustfmt::skip]
pub fn nbx_jets() -> impl Iterator<Item = HotEntry> {
    [
        NBX_ONE_JETS,
        NBX_TWO_JETS,
        NBX_THREE_JETS,
        NBX_FOUR_JETS,
        NBX_FIVE_JETS,
        NBX_SIX_JETS,
        NBX_EIGHT_JETS,
        NBX_MEMORY_V0_V1_JETS,
        NBX_COMPUTE_V0_V1_JETS,
        NBX_MEMORY_V2_JETS,
        NBX_COMPUTE_V2_JETS,
        NBX_HOON_JETS,
        NBX_PROVER_JETS,
        //NBX_ZOON_JETS,
    ]
    .map(|v| v.iter().copied())
    .into_iter()
    .flatten()
    .map(|(a, b, c)| (heap_xor(a), b, c))
}
