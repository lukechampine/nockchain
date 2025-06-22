use std::sync::atomic::{AtomicBool, Ordering};

use either::Either::*;
use nockvm::interpreter::Context;
use nockvm::jets::hot::{HotEntry, K_138};
use nockvm::jets::util::slot;
use nockvm::jets::{JetErr, Result};
use nockvm::noun::*;

mod eight;
mod five;
mod four;
mod one;
mod prover_memory;
mod six;
mod three;
mod two;
mod utils;

use eight::*;
use five::*;
use four::*;
use one::*;
use prover_memory::*;
use six::*;
use three::*;
use two::*;

use super::utils::jet_err;

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
        pub fn $name(context: &mut Context, subject: Noun) -> Result {
            jet_option!($imp => $($l)*: {
                $imp(context, subject)
            })
        }
    };
    ($name:ident => $imp:ident $($l:lifetime)*$(,)?) => {
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
    build_jet => build,
    precompute_ntts_jet => precompute_ntts,
    turn_coseword_jet => turn_coseword,
    pad_jet => pad,
    prove_fri_door_jet => prove_fri_door 'raw 'jam 'create_jam_dir,
    prove_commit_jet => prove_commit 'raw,// 'jam 'create_jam_dir,
    //absorb_proof_objects_jet => absorb_proof_objects //'jam 'create_jam_dir,
}

pub const NBX_ONE_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"range"),
        ],
        1,
        range_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ave"),
            Left(b"zero-extend"),
        ],
        1,
        zero_extend_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ave"),
            Left(b"weld-step"),
        ],
        1,
        weld_step_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"do-init-mary"),
        ],
        1,
        do_init_mary_jet,
    ),
];

pub const NBX_TWO_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"mp-substitute-ultra"),
        ],
        1,
        mp_substitute_ultra_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"fp-ntt"),
        ],
        1,
        fp_ntt_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"fp-fft"),
        ],
        1,
        fp_fft_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"fp-ifft"),
        ],
        1,
        fp_ifft_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"bpeval-lift"),
        ],
        1,
        bpeval_lift_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"turn-coseword"),
        ],
        1,
        turn_coseword_jet,
    ),
];

pub const NBX_THREE_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"shape"),
            Left(b"leaf-sequence"),
        ],
        1,
        leaf_sequence_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"tip5-lib"),
            Left(b"hash-noun-varlen"),
        ],
        1,
        hash_noun_varlen_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"tip5-lib"),
            Left(b"hash-varlen"),
        ],
        1,
        hash_varlen_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"tip5-lib"),
            Left(b"hash-10"),
        ],
        1,
        hash_10_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"tip5-lib"),
            Left(b"hash-ten-cell"),
        ],
        1,
        hash_ten_cell_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"tip5-lib"),
            Left(b"hash-hashable"),
        ],
        1,
        hash_hashable_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"merkle"),
            Left(b"bp-build-merk-heap"),
        ],
        1,
        bp_build_merk_heap_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"merkle"),
            Left(b"build-merk-heap"),
        ],
        1,
        build_merk_heap_jet,
    ),
];

// TODO: figure out how to build a core for `tog`, and enable this.
pub const NBX_FOUR_JETS: &[HotEntry] = &[/*(
    &[
        K_138,
        Left(b"one"),
        Left(b"two"),
        Left(b"tri"),
        Left(b"qua"),
        Left(b"pen"),
        Left(b"zeke"),
        Left(b"ext-field"),
        Left(b"misc-lib"),
        Left(b"proof-lib"),
        Left(b"absorb-proof-objects"),
    ],
    1,
    absorb_proof_objects_jet,
)*/];

pub const NBX_FIVE_JETS: &[HotEntry] = &[(
    &[
        K_138,
        Left(b"one"),
        Left(b"two"),
        Left(b"tri"),
        Left(b"qua"),
        Left(b"pen"),
        Left(b"zeke"),
        Left(b"ext-field"),
        Left(b"misc-lib"),
        Left(b"proof-lib"),
        Left(b"utils"),
        Left(b"constraint-util"),
        Left(b"pstack"),
        Left(b"push-all"),
    ],
    1,
    pstack_push_all_jet,
)];

pub const NBX_SIX_JETS: &[HotEntry] = &[
    /*(
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"proof-lib"),
            Left(b"utils"),
            Left(b"fri"),
            Left(b"fri-door"),
            Left(b"prove"),
        ],
        1,
        prove_fri_door_jet,
    ),*/
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"proof-lib"),
            Left(b"utils"),
            Left(b"fri"),
            Left(b"fri-door"),
            Left(b"prove-commit"),
        ],
        1,
        prove_commit_jet,
    ),
];

pub const NBX_EIGHT_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"proof-lib"),
            Left(b"utils"),
            Left(b"fri"),
            Left(b"table-lib"),
            Left(b"stark-core"),
            Left(b"compute-composition-poly"),
        ],
        1,
        compute_composition_poly_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"proof-lib"),
            Left(b"utils"),
            Left(b"fri"),
            Left(b"table-lib"),
            Left(b"stark-core"),
            Left(b"compute-deep"),
        ],
        1,
        compute_deep_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"zeke"),
            Left(b"ext-field"),
            Left(b"misc-lib"),
            Left(b"proof-lib"),
            Left(b"utils"),
            Left(b"fri"),
            Left(b"table-lib"),
            Left(b"stark-core"),
            Left(b"precompute-ntts"),
        ],
        1,
        precompute_ntts_jet,
    ),
];

pub const NBX_MEMORY_JETS: &[HotEntry] = &[
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"memory-table"),
            Left(b"rna-bfta"),
        ],
        1,
        rna_bfta_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"memory-table"),
            Left(b"funcs"),
            Left(b"build"),
        ],
        1,
        build_jet,
    ),
    (
        &[
            K_138,
            Left(b"one"),
            Left(b"two"),
            Left(b"tri"),
            Left(b"qua"),
            Left(b"pen"),
            Left(b"memory-table"),
            Left(b"funcs"),
            Left(b"pad"),
        ],
        1,
        pad_jet,
    ),
];

pub fn nbx_jets() -> impl Iterator<Item = HotEntry> {
    [
        NBX_ONE_JETS,
        NBX_TWO_JETS,
        NBX_THREE_JETS,
        NBX_FOUR_JETS,
        NBX_FIVE_JETS,
        NBX_SIX_JETS,
        NBX_EIGHT_JETS,
        NBX_MEMORY_JETS,
    ]
    .map(|v| v.iter().copied())
    .into_iter()
    .flatten()
}
