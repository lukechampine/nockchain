use std::simd::cmp::SimdPartialOrd;
use std::simd::{u64x2, u64x8};

use const_for::const_for;

use crate::base::{badd_simd_x2, montiply_simd_x2, montiply_simd_x8};
use crate::melt::Melt;
use crate::tip5::{
    mds_generated, LOOKUP_TABLE, MELT_ONE_POW_7, NUM_ROUNDS, NUM_SPLIT_AND_LOOKUP,
    ROUND_CONSTANTS2, STATE_SIZE,
};

#[inline(always)]
pub fn permute_last_x2(input: [Melt; 16], other_input: [Melt; 16]) -> ([Melt; 5], [Melt; 5]) {
    let (mut sponge, mut other_sponge) = sbox_layer_x2(&input, &other_input);
    rcs_layer_x2(&mut sponge, &mut other_sponge, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        let (new_sponge, new_other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
        sponge = new_sponge;
        other_sponge = new_other_sponge;

        rcs_layer_x2(&mut sponge, &mut other_sponge, i);
    });

    let (sponge, other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
    rcs_layer_last_x2(&sponge, &other_sponge)
}

#[inline(always)]
pub fn sbox_layer_x2(
    state1: &[Melt; STATE_SIZE],
    state2: &[Melt; STATE_SIZE],
) -> ([Melt; STATE_SIZE], [Melt; STATE_SIZE]) {
    let mut res1: [Melt; STATE_SIZE] = [Melt(0); STATE_SIZE];
    let mut res2: [Melt; STATE_SIZE] = [Melt(0); STATE_SIZE];

    // Process lookup table elements (0-3) for both sponges
    const_for!(i in 0..NUM_SPLIT_AND_LOOKUP => {
        let mut bytes1 = state1[i].0.to_le_bytes();
        let mut bytes2 = state2[i].0.to_le_bytes();
        const_for!(j in 0..8 => {
            bytes1[j] = LOOKUP_TABLE[bytes1[j] as usize];
            bytes2[j] = LOOKUP_TABLE[bytes2[j] as usize];
        });
        res1[i] = Melt(u64::from_le_bytes(bytes1));
        res2[i] = Melt(u64::from_le_bytes(bytes2));
    });

    // Process elements 4-7 from both sponges with SIMD x8
    let s1_a = u64x8::from_array([
        state1[4].0, state1[5].0, state1[6].0, state1[7].0, state2[4].0, state2[5].0, state2[6].0,
        state2[7].0,
    ]);
    let s2_a = montiply_simd_x8(s1_a, s1_a);
    let s4_a = montiply_simd_x8(s2_a, s2_a);
    let s7_a = montiply_simd_x8(montiply_simd_x8(s1_a, s2_a), s4_a);
    let results_a = s7_a.as_array();

    res1[4].0 = results_a[0];
    res1[5].0 = results_a[1];
    res1[6].0 = results_a[2];
    res1[7].0 = results_a[3];
    res2[4].0 = results_a[4];
    res2[5].0 = results_a[5];
    res2[6].0 = results_a[6];
    res2[7].0 = results_a[7];

    // Process elements 8-11 from both sponges with SIMD x8
    let s1_b = u64x8::from_array([
        state1[8].0, state1[9].0, state1[10].0, state1[11].0, state2[8].0, state2[9].0,
        state2[10].0, state2[11].0,
    ]);
    let s2_b = montiply_simd_x8(s1_b, s1_b);
    let s4_b = montiply_simd_x8(s2_b, s2_b);
    let s7_b = montiply_simd_x8(montiply_simd_x8(s1_b, s2_b), s4_b);
    let results_b = s7_b.as_array();

    res1[8].0 = results_b[0];
    res1[9].0 = results_b[1];
    res1[10].0 = results_b[2];
    res1[11].0 = results_b[3];
    res2[8].0 = results_b[4];
    res2[9].0 = results_b[5];
    res2[10].0 = results_b[6];
    res2[11].0 = results_b[7];

    // Process elements 12-15 from both sponges with SIMD x8
    let s1_c = u64x8::from_array([
        state1[12].0, state1[13].0, state1[14].0, state1[15].0, state2[12].0, state2[13].0,
        state2[14].0, state2[15].0,
    ]);
    let s2_c = montiply_simd_x8(s1_c, s1_c);
    let s4_c = montiply_simd_x8(s2_c, s2_c);
    let s7_c = montiply_simd_x8(montiply_simd_x8(s1_c, s2_c), s4_c);
    let results_c = s7_c.as_array();

    res1[12].0 = results_c[0];
    res1[13].0 = results_c[1];
    res1[14].0 = results_c[2];
    res1[15].0 = results_c[3];
    res2[12].0 = results_c[4];
    res2[13].0 = results_c[5];
    res2[14].0 = results_c[6];
    res2[15].0 = results_c[7];

    (res1, res2)
}

#[inline(always)]
pub(crate) fn sbox_layer_fixed_x2(
    input1: &[Melt; 10],
    input2: &[Melt; 10],
) -> ([Melt; STATE_SIZE], [Melt; STATE_SIZE]) {
    let mut res1: [Melt; STATE_SIZE] = [
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
    ];

    let mut res2: [Melt; STATE_SIZE] = [
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        Melt(0),
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
    ];

    // Process lookup table elements (0-3) for both inputs
    const_for!(i in 0..NUM_SPLIT_AND_LOOKUP => {
        let mut bytes1 = input1[i].0.to_le_bytes();
        let mut bytes2 = input2[i].0.to_le_bytes();
        const_for!(j in 0..8 => {
            bytes1[j] = LOOKUP_TABLE[bytes1[j] as usize];
            bytes2[j] = LOOKUP_TABLE[bytes2[j] as usize];
        });
        res1[i] = Melt(u64::from_le_bytes(bytes1));
        res2[i] = Melt(u64::from_le_bytes(bytes2));
    });

    // Process elements 4-7 from both inputs with SIMD x8
    let s1_a = u64x8::from_array([
        input1[4].0, input1[5].0, input1[6].0, input1[7].0, input2[4].0, input2[5].0, input2[6].0,
        input2[7].0,
    ]);
    let s2_a = montiply_simd_x8(s1_a, s1_a);
    let s4_a = montiply_simd_x8(s2_a, s2_a);
    let s7_a = montiply_simd_x8(montiply_simd_x8(s1_a, s2_a), s4_a);
    let results_a = s7_a.as_array();

    res1[4].0 = results_a[0];
    res1[5].0 = results_a[1];
    res1[6].0 = results_a[2];
    res1[7].0 = results_a[3];
    res2[4].0 = results_a[4];
    res2[5].0 = results_a[5];
    res2[6].0 = results_a[6];
    res2[7].0 = results_a[7];

    // Process elements 8-9 from both inputs with two SIMD x2 calls
    let s1_b1 = u64x2::from_array([input1[8].0, input1[9].0]);
    let s2_b1 = montiply_simd_x2(s1_b1, s1_b1);
    let s4_b1 = montiply_simd_x2(s2_b1, s2_b1);
    let s7_b1 = montiply_simd_x2(montiply_simd_x2(s1_b1, s2_b1), s4_b1);
    let results_b1 = s7_b1.as_array();

    res1[8].0 = results_b1[0];
    res1[9].0 = results_b1[1];

    let s1_b2 = u64x2::from_array([input2[8].0, input2[9].0]);
    let s2_b2 = montiply_simd_x2(s1_b2, s1_b2);
    let s4_b2 = montiply_simd_x2(s2_b2, s2_b2);
    let s7_b2 = montiply_simd_x2(montiply_simd_x2(s1_b2, s2_b2), s4_b2);
    let results_b2 = s7_b2.as_array();

    res2[8].0 = results_b2[0];
    res2[9].0 = results_b2[1];

    (res1, res2)
}

#[inline(always)]
pub fn permute_intermediate_x2(input: &mut [Melt; 16], other_input: &mut [Melt; 16]) {
    let (mut sponge, mut other_sponge) = sbox_layer_x2(&input, &other_input);
    rcs_layer_x2(&mut sponge, &mut other_sponge, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        let (new_sponge, new_other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
        sponge = new_sponge;
        other_sponge = new_other_sponge;

        rcs_layer_x2(&mut sponge, &mut other_sponge, i);
    });

    let (mut sponge, mut other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
    rcs_layer_intermediate(&mut sponge, &mut other_sponge, NUM_ROUNDS - 1);
    *input = sponge;
    *other_input = other_sponge;
}

#[inline(always)]
pub fn permute_fixed_x2(input: &[Melt; 10], other_input: &[Melt; 10]) -> ([Melt; 5], [Melt; 5]) {
    let (mut sponge, mut other_sponge) = sbox_layer_fixed_x2(&input, &other_input);

    rcs_layer_x2(&mut sponge, &mut other_sponge, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        let (new_sponge, new_other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
        sponge = new_sponge;
        other_sponge = new_other_sponge;

        rcs_layer_x2(&mut sponge, &mut other_sponge, i);
    });

    let (sponge, other_sponge) = sbox_layer_x2(&sponge, &other_sponge);
    rcs_layer_last_x2(&sponge, &other_sponge)
}

#[inline(always)]
pub fn rcs_layer_last_x2(sponge: &[Melt; 16], other_sponge: &[Melt; 16]) -> ([Melt; 5], [Melt; 5]) {
    let mut arrays: [[u32; STATE_SIZE]; 4] = [[0; STATE_SIZE]; 4];

    for i in 0..STATE_SIZE {
        let b0 = sponge[i].0;
        let b1 = other_sponge[i].0;

        arrays[0][i] = b0 as u32; // lo[0]
        arrays[1][i] = b1 as u32; // lo[1]
        arrays[2][i] = (b0 >> 32) as u32; // hi[0]
        arrays[3][i] = (b1 >> 32) as u32; // hi[1]
    }

    let results = mds_generated::generated_last_simd_x4(&arrays);

    let mut result0 = [Melt(0); 5];
    let mut result1 = [Melt(0); 5];

    for r in 0..5 {
        // Extract lo and hi results for both sponges
        let lo_results = u64x2::from_array([results[r].as_array()[0], results[r].as_array()[1]]);
        let hi_results = u64x2::from_array([results[r].as_array()[2], results[r].as_array()[3]]);

        let a_simd = lo_results >> u64x2::splat(4);
        let b_simd = hi_results << u64x2::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x2::splat(1), u64x2::splat(0));

        let s_hi_simd = hi_results >> u64x2::splat(36);
        let d_simd = s_hi_simd * u64x2::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x2::splat(1), u64x2::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x2::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;

        let final_result = badd_simd_x2(
            u64x2::splat(ROUND_CONSTANTS2[(NUM_ROUNDS - 1) * STATE_SIZE + r].0),
            linear_result,
        );

        result0[r].0 = final_result.as_array()[0];
        result1[r].0 = final_result.as_array()[1];
    }

    (result0, result1)
}

#[inline(always)]
fn rcs_layer_intermediate(
    sponge: &mut [Melt; 16],
    other_sponge: &mut [Melt; 16],
    round_num: usize,
) {
    let mut arrays: [[u32; STATE_SIZE]; 4] = [[0; STATE_SIZE]; 4];

    for i in 0..STATE_SIZE {
        let b0 = sponge[i].0;
        let b1 = other_sponge[i].0;

        arrays[0][i] = b0 as u32; // lo[0]
        arrays[1][i] = b1 as u32; // lo[1]
        arrays[2][i] = (b0 >> 32) as u32; // hi[0]
        arrays[3][i] = (b1 >> 32) as u32; // hi[1]
    }

    let results = mds_generated::generated_intermediate_simd_x4(&arrays);

    for r in 10..STATE_SIZE {
        // Extract lo and hi results for both sponges
        let lo_simd =
            u64x2::from_array([results[r - 10].as_array()[0], results[r - 10].as_array()[1]]);
        let hi_simd =
            u64x2::from_array([results[r - 10].as_array()[2], results[r - 10].as_array()[3]]);

        let a_simd = lo_simd >> u64x2::splat(4);
        let b_simd = hi_simd << u64x2::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x2::splat(1), u64x2::splat(0));

        let s_hi_simd = hi_simd >> u64x2::splat(36);
        let d_simd = s_hi_simd * u64x2::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x2::splat(1), u64x2::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x2::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;

        let final_result = badd_simd_x2(
            u64x2::splat(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0),
            linear_result,
        );

        sponge[r].0 = final_result.as_array()[0];
        other_sponge[r].0 = final_result.as_array()[1];
    }
}

#[inline(always)]
pub fn rcs_layer_x2(sponge: &mut [Melt; 16], other_sponge: &mut [Melt; 16], round_num: usize) {
    let mut arrays: [[u32; STATE_SIZE]; 4] = [[0; STATE_SIZE]; 4];

    for i in 0..STATE_SIZE {
        let b0 = sponge[i].0;
        let b1 = other_sponge[i].0;

        arrays[0][i] = b0 as u32; // lo[0]
        arrays[1][i] = b1 as u32; // lo[1]
        arrays[2][i] = (b0 >> 32) as u32; // hi[0]
        arrays[3][i] = (b1 >> 32) as u32; // hi[1]
    }

    let results = mds_generated::generated_simd_x4(&arrays);

    for r in 0..STATE_SIZE {
        // Extract lo and hi results for both sponges
        let lo_simd = u64x2::from_array([results[r].as_array()[0], results[r].as_array()[1]]);
        let hi_simd = u64x2::from_array([results[r].as_array()[2], results[r].as_array()[3]]);

        let a_simd = lo_simd >> u64x2::splat(4);
        let b_simd = hi_simd << u64x2::splat(28);
        let s_lo_simd = a_simd + b_simd;

        // Overflow detection - convert masks to 0 or 1
        let o1_simd = a_simd.simd_gt(s_lo_simd) | b_simd.simd_gt(s_lo_simd);
        let o1_count = o1_simd.select(u64x2::splat(1), u64x2::splat(0));

        let s_hi_simd = hi_simd >> u64x2::splat(36);
        let d_simd = s_hi_simd * u64x2::splat(0xffff_ffffu64);
        let ret_simd = s_lo_simd + d_simd;

        // Overflow detection
        let o2_simd = s_lo_simd.simd_gt(ret_simd) | d_simd.simd_gt(ret_simd);
        let o2_count = o2_simd.select(u64x2::splat(1), u64x2::splat(0));

        // Combine overflow corrections
        let overflow_simd = o1_count + o2_count;
        let correction_simd = u64x2::splat(0xffff_ffffu64) * overflow_simd;

        let linear_result = ret_simd + correction_simd;

        let final_result = badd_simd_x2(
            u64x2::splat(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0),
            linear_result,
        );

        sponge[r].0 = final_result.as_array()[0];
        other_sponge[r].0 = final_result.as_array()[1];
    }
}
