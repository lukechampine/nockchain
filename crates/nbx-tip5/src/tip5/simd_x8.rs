use std::simd::cmp::SimdPartialOrd;
use std::simd::u64x8;

use const_for::const_for;

use crate::base::{badd_simd, montiply_simd_x8};
use crate::melt::Melt;
use crate::tip5::{
    mds_generated, LOOKUP_TABLE, MELT_ONE_POW_7, NUM_ROUNDS, NUM_SPLIT_AND_LOOKUP,
    ROUND_CONSTANTS2, STATE_SIZE,
};

#[inline(always)]
pub fn permute_last_x8(input: [[Melt; 16]; 8]) -> [[Melt; 5]; 8] {
    let mut sponges = sbox_layer_x8_initial(&input);
    rcs_layer_x8(&mut sponges, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        sbox_layer_x8(&mut sponges);
        rcs_layer_x8(&mut sponges, i);
    });

    sbox_layer_x8(&mut sponges);
    rcs_layer_last_x8(&mut sponges)
}

#[inline(always)]
pub fn sbox_layer_x8_initial(input: &[[Melt; 16]; 8]) -> [u64x8; 16] {
    let mut sponges: [u64x8; 16] = [u64x8::splat(0); 16];

    // Process lookup table elements (0-3)
    for i in 0..NUM_SPLIT_AND_LOOKUP {
        let mut results = [0u64; 8];
        for j in 0..8 {
            let mut bytes = input[j][i].0.to_le_bytes();
            for k in 0..8 {
                bytes[k] = LOOKUP_TABLE[bytes[k] as usize];
            }
            results[j] = u64::from_le_bytes(bytes);
        }
        sponges[i] = u64x8::from_array(results);
    }

    // Process elements 4-15 using SIMD
    for i in NUM_SPLIT_AND_LOOKUP..STATE_SIZE {
        let s1 = u64x8::from_array([
            input[0][i].0, input[1][i].0, input[2][i].0, input[3][i].0, input[4][i].0,
            input[5][i].0, input[6][i].0, input[7][i].0,
        ]);
        let s2 = montiply_simd_x8(s1, s1);
        let s4 = montiply_simd_x8(s2, s2);
        sponges[i] = montiply_simd_x8(montiply_simd_x8(s1, s2), s4);
    }

    sponges
}

#[inline(always)]
pub fn sbox_layer_x8(sponges: &mut [u64x8; 16]) {
    // Process lookup table elements (0-3)
    for i in 0..NUM_SPLIT_AND_LOOKUP {
        let values = sponges[i].as_array();
        let mut results = [0u64; 8];

        for j in 0..8 {
            let mut bytes = values[j].to_le_bytes();
            for k in 0..8 {
                bytes[k] = LOOKUP_TABLE[bytes[k] as usize];
            }
            results[j] = u64::from_le_bytes(bytes);
        }

        sponges[i] = u64x8::from_array(results);
    }

    // Process elements 4-15 using SIMD (already in x8 format)
    for j in NUM_SPLIT_AND_LOOKUP..STATE_SIZE {
        let s1 = sponges[j];
        let s2 = montiply_simd_x8(s1, s1);
        let s4 = montiply_simd_x8(s2, s2);
        sponges[j] = montiply_simd_x8(montiply_simd_x8(s1, s2), s4);
    }
}

#[inline(always)]
pub fn permute_intermediate_x8(input: &mut [[Melt; 16]; 8]) {
    let mut sponges = sbox_layer_x8_initial(&input);
    rcs_layer_x8(&mut sponges, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        sbox_layer_x8(&mut sponges);
        rcs_layer_x8(&mut sponges, i);
    });

    sbox_layer_x8(&mut sponges);
    rcs_layer_intermediate(&mut sponges, NUM_ROUNDS - 1);

    // Transpose back from [u64x8; 16] to [[Melt; 16]; 8]
    for j in 0..16 {
        let values = sponges[j].to_array();
        input[0][j] = Melt(values[0]);
        input[1][j] = Melt(values[1]);
        input[2][j] = Melt(values[2]);
        input[3][j] = Melt(values[3]);
        input[4][j] = Melt(values[4]);
        input[5][j] = Melt(values[5]);
        input[6][j] = Melt(values[6]);
        input[7][j] = Melt(values[7]);
    }
}

#[inline(always)]
pub fn permute_fixed_x8(input: [[Melt; 10]; 8]) -> [[Melt; 5]; 8] {
    let mut sponges = sbox_layer_fixed_x8_initial(&input);
    rcs_layer_x8(&mut sponges, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
       sbox_layer_x8(&mut sponges);
        rcs_layer_x8(&mut sponges, i);
    });

    sbox_layer_x8(&mut sponges);
    rcs_layer_last_x8(&mut sponges)
}

#[inline(always)]
pub fn sbox_layer_fixed_x8_initial(input: &[[Melt; 10]; 8]) -> [u64x8; 16] {
    let mut sponges: [u64x8; 16] = [u64x8::splat(0); 16];

    // Process lookup table elements (0-3)
    for i in 0..NUM_SPLIT_AND_LOOKUP {
        let mut results = [0u64; 8];
        for j in 0..8 {
            let mut bytes = input[j][i].0.to_le_bytes();
            for k in 0..8 {
                bytes[k] = LOOKUP_TABLE[bytes[k] as usize];
            }
            results[j] = u64::from_le_bytes(bytes);
        }
        sponges[i] = u64x8::from_array(results);
    }

    // Process elements 4-9 using SIMD
    for i in NUM_SPLIT_AND_LOOKUP..10 {
        let s1 = u64x8::from_array([
            input[0][i].0, input[1][i].0, input[2][i].0, input[3][i].0, input[4][i].0,
            input[5][i].0, input[6][i].0, input[7][i].0,
        ]);
        let s2 = montiply_simd_x8(s1, s1);
        let s4 = montiply_simd_x8(s2, s2);
        sponges[i] = montiply_simd_x8(montiply_simd_x8(s1, s2), s4);
    }

    // Elements 10-15 are pre-computed MELT_ONE_POW_7
    for i in 10..STATE_SIZE {
        sponges[i] = u64x8::splat(MELT_ONE_POW_7.0);
    }

    sponges
}

#[inline(always)]
fn rcs_layer_x8(sponges: &mut [u64x8; 16], round_num: usize) {
    let mut lo: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];
    let mut hi: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];

    for i in 0..STATE_SIZE {
        let simd_vals = sponges[i];
        let lo_simd = simd_vals & u64x8::splat(0xFFFF_FFFF);
        let hi_simd = simd_vals >> u64x8::splat(32);

        let lo_array = lo_simd.as_array();
        let hi_array = hi_simd.as_array();

        for j in 0..8 {
            lo[j][i] = lo_array[j] as u32;
            hi[j][i] = hi_array[j] as u32;
        }
    }

    // Use SIMD for MDS operations
    let lo_results = mds_generated::generated_simd_x8(&lo);
    let hi_results = mds_generated::generated_simd_x8(&hi);

    // Scalar post-processing for each result
    for r in 0..STATE_SIZE {
        // Pack values into SIMD
        let lo_simd = lo_results[r];
        let hi_simd = hi_results[r];

        let a_simd = lo_simd >> u64x8::splat(4);
        let b_simd = hi_simd << u64x8::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x8::splat(1), u64x8::splat(0));

        let s_hi_simd = hi_simd >> u64x8::splat(36);
        let d_simd = s_hi_simd * u64x8::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x8::splat(1), u64x8::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x8::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;
        sponges[r] = badd_simd(
            u64x8::splat(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0),
            linear_result,
        );
    }
}

#[inline(always)]
fn rcs_layer_last_x8(sponges: &[u64x8; 16]) -> [[Melt; 5]; 8] {
    let mut lo: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];
    let mut hi: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];

    for i in 0..STATE_SIZE {
        let simd_vals = sponges[i];
        let lo_simd = simd_vals & u64x8::splat(0xFFFF_FFFF);
        let hi_simd = simd_vals >> u64x8::splat(32);

        let lo_array = lo_simd.as_array();
        let hi_array = hi_simd.as_array();

        for j in 0..8 {
            lo[j][i] = lo_array[j] as u32;
            hi[j][i] = hi_array[j] as u32;
        }
    }

    let lo_results = mds_generated::generated_last_simd_x8(&lo);
    let hi_results = mds_generated::generated_last_simd_x8(&hi);

    let mut results = [[Melt(0); 5]; 8];

    for r in 0..5 {
        // Pack values into SIMD
        let lo_simd = lo_results[r];
        let hi_simd = hi_results[r];

        let a_simd = lo_simd >> u64x8::splat(4);
        let b_simd = hi_simd << u64x8::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x8::splat(1), u64x8::splat(0));

        let s_hi_simd = hi_simd >> u64x8::splat(36);
        let d_simd = s_hi_simd * u64x8::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x8::splat(1), u64x8::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x8::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;
        let final_result = badd_simd(
            u64x8::splat(ROUND_CONSTANTS2[(NUM_ROUNDS - 1) * STATE_SIZE + r].0),
            linear_result,
        )
        .to_array();

        results[0][r].0 = final_result[0];
        results[1][r].0 = final_result[1];
        results[2][r].0 = final_result[2];
        results[3][r].0 = final_result[3];
        results[4][r].0 = final_result[4];
        results[5][r].0 = final_result[5];
        results[6][r].0 = final_result[6];
        results[7][r].0 = final_result[7];
    }

    results
}

#[inline(always)]
fn rcs_layer_intermediate(sponges: &mut [u64x8; 16], round_num: usize) {
    let mut lo: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];
    let mut hi: [[u32; STATE_SIZE]; 8] = [[0; STATE_SIZE]; 8];

    for i in 0..STATE_SIZE {
        let simd_vals = sponges[i];
        let lo_simd = simd_vals & u64x8::splat(0xFFFF_FFFF);
        let hi_simd = simd_vals >> u64x8::splat(32);

        let lo_array = lo_simd.as_array();
        let hi_array = hi_simd.as_array();

        for j in 0..8 {
            lo[j][i] = lo_array[j] as u32;
            hi[j][i] = hi_array[j] as u32;
        }
    }

    let lo_results = mds_generated::generated_intermediate_simd_x8(&lo);
    let hi_results = mds_generated::generated_intermediate_simd_x8(&hi);

    // Because the first 10 elements will be overwritten in the next step, there is no need to add them
    for r in 10..STATE_SIZE {
        // Pack values into SIMD
        let lo_simd = lo_results[r - 10];
        let hi_simd = hi_results[r - 10];

        let a_simd = lo_simd >> u64x8::splat(4);
        let b_simd = hi_simd << u64x8::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x8::splat(1), u64x8::splat(0));

        let s_hi_simd = hi_simd >> u64x8::splat(36);
        let d_simd = s_hi_simd * u64x8::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x8::splat(1), u64x8::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x8::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;
        sponges[r] = badd_simd(
            u64x8::splat(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0),
            linear_result,
        );
    }
}
