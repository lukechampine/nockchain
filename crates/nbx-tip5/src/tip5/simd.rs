use std::simd::{u64x2, u64x8};

use const_for::const_for;

use crate::base::{badd, montiply_simd_x2, montiply_simd_x8};
use crate::melt::Melt;
use crate::tip5::{
    mds_generated, LOOKUP_TABLE, MELT_ONE_POW_7, NUM_ROUNDS, NUM_SPLIT_AND_LOOKUP,
    ROUND_CONSTANTS2, STATE_SIZE,
};

pub fn permute(sponge: &mut [Melt; 16]) {
    const_for!(i in 0..NUM_ROUNDS => {
        let mut a = sbox_layer(sponge);
        rcs_layer(&mut a, i);
        *sponge = a;
    });
}

#[inline(always)]
pub fn permute_intermediate(sponge: &mut [Melt; 16]) {
    const_for!(i in 0..NUM_ROUNDS-1 => {
        let mut a = sbox_layer(sponge);
        rcs_layer(&mut a, i);
        *sponge = a;
    });

    let mut a = sbox_layer(sponge);
    rcs_layer_intermediate_permute(&mut a, NUM_ROUNDS - 1);
    *sponge = a;
}

#[inline(always)]
pub fn permute_last(sponge: [Melt; 16]) -> [Melt; 5] {
    let mut sponge = sponge;

    const_for!(i in 0..NUM_ROUNDS-1 => {
        let mut a = sbox_layer(&mut sponge);
        rcs_layer(&mut a, i);
        sponge = a;
    });

    rcs_layer_last_permute(sbox_layer(&mut sponge))
}

#[inline(always)]
pub fn permute_fixed(input: &[Melt; 10]) -> [Melt; 5] {
    let mut sponge = sbox_layer_fixed(input);
    rcs_layer(&mut sponge, 0);

    const_for!(i in 1..NUM_ROUNDS-1 => {
        let mut a = sbox_layer(&mut sponge);
        rcs_layer(&mut a, i);
        sponge = a;
    });

    rcs_layer_last_permute(sbox_layer(&mut sponge))
}

#[inline(always)]
pub(crate) fn sbox_layer(state: &[Melt; STATE_SIZE]) -> [Melt; STATE_SIZE] {
    let mut res: [Melt; STATE_SIZE] = [Melt(0); STATE_SIZE];

    const_for!(i in 0..NUM_SPLIT_AND_LOOKUP => {
        let mut bytes = state[i].0.to_le_bytes();
        const_for!(i in 0..8 => {
            bytes[i] = LOOKUP_TABLE[bytes[i] as usize];
        });
        res[i] = Melt(u64::from_le_bytes(bytes));
    });

    // Process 8 elements with SIMD x8
    let s1_arr = [
        state[4].0, state[5].0, state[6].0, state[7].0, state[8].0, state[9].0, state[10].0,
        state[11].0,
    ];
    let s1 = u64x8::from_array(s1_arr);
    let s2 = montiply_simd_x8(s1, s1);
    let s4 = montiply_simd_x8(s2, s2);
    let s7 = montiply_simd_x8(montiply_simd_x8(s1, s2), s4);
    let results = s7.as_array();
    for i in 0..8 {
        res[4 + i].0 = results[i];
    }

    // Process last 4 elements with SIMD x2
    let s1_arr1 = [state[12].0, state[13].0];
    let s1_1 = u64x2::from_array(s1_arr1);
    let s2_1 = montiply_simd_x2(s1_1, s1_1);
    let s4_1 = montiply_simd_x2(s2_1, s2_1);
    let s7_1 = montiply_simd_x2(montiply_simd_x2(s1_1, s2_1), s4_1);
    let results1 = s7_1.as_array();
    res[12].0 = results1[0];
    res[13].0 = results1[1];

    let s1_arr2 = [state[14].0, state[15].0];
    let s1_2 = u64x2::from_array(s1_arr2);
    let s2_2 = montiply_simd_x2(s1_2, s1_2);
    let s4_2 = montiply_simd_x2(s2_2, s2_2);
    let s7_2 = montiply_simd_x2(montiply_simd_x2(s1_2, s2_2), s4_2);
    let results2 = s7_2.as_array();
    res[14].0 = results2[0];
    res[15].0 = results2[1];

    res
}

#[inline(always)]
pub(crate) fn sbox_layer_fixed(input: &[Melt; 10]) -> [Melt; STATE_SIZE] {
    let mut res: [Melt; STATE_SIZE] = [
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
        // Because RATE is only 10, there are 6 elements remaining that are set to Melt::one(). The
        //  pow(7) of these values are pre-computed.
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
        MELT_ONE_POW_7,
    ];

    const_for!(i in 0..NUM_SPLIT_AND_LOOKUP => {
        let mut bytes = input[i].0.to_le_bytes();
        const_for!(i in 0..8 => {
            bytes[i] = LOOKUP_TABLE[bytes[i] as usize];
        });
        res[i] = Melt(u64::from_le_bytes(bytes));
    });

    // Only process the pow(7) for the elements up to Melt::one()
    // Process elements 4-5 with SIMD x2
    let s1_1 = u64x2::from_array([input[4].0, input[5].0]);
    let s2_1 = montiply_simd_x2(s1_1, s1_1);
    let s4_1 = montiply_simd_x2(s2_1, s2_1);
    let s7_1 = montiply_simd_x2(montiply_simd_x2(s1_1, s2_1), s4_1);
    let results1 = s7_1.as_array();
    res[4].0 = results1[0];
    res[5].0 = results1[1];

    // Process elements 6-7 with SIMD x2
    let s1_2 = u64x2::from_array([input[6].0, input[7].0]);
    let s2_2 = montiply_simd_x2(s1_2, s1_2);
    let s4_2 = montiply_simd_x2(s2_2, s2_2);
    let s7_2 = montiply_simd_x2(montiply_simd_x2(s1_2, s2_2), s4_2);
    let results2 = s7_2.as_array();
    res[6].0 = results2[0];
    res[7].0 = results2[1];

    // Process elements 8-9 with SIMD x2
    let s1_3 = u64x2::from_array([input[8].0, input[9].0]);
    let s2_3 = montiply_simd_x2(s1_3, s1_3);
    let s4_3 = montiply_simd_x2(s2_3, s2_3);
    let s7_3 = montiply_simd_x2(montiply_simd_x2(s1_3, s2_3), s4_3);
    let results3 = s7_3.as_array();
    res[8].0 = results3[0];
    res[9].0 = results3[1];

    res
}

#[inline(always)]
fn rcs_layer(state: &mut [Melt; 16], round_num: usize) {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let results = mds_generated::generated_simd_x2(&[hi, lo]);

    const_for!(r in 0..STATE_SIZE => {
         let results = results[r].as_array();

        let hi = results[0];
        let lo = results[1];

        let a = lo >> 4;
        let b = hi << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi >> 36;
        let d = s_hi * 0xffff_ffffu64;
        let ret = s_lo.wrapping_add(d);
        // NOTE: this vectorizes much better than overflow flag
        let o2 = s_lo > ret || d > ret;
        let linear_result = ret + 0xffff_ffffu64 * ((o1 as u64) + (o2 as u64));

        state[r].0 = badd(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0, linear_result);
    });
}

#[inline(always)]
fn rcs_layer_intermediate_permute(state: &mut [Melt; 16], round_num: usize) {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let results = mds_generated::generated_intermediate_simd_x2(&[hi, lo]);

    // Because the first 10 elements will be overwritten in the next step, there is no need to compute them
    const_for!(r in 10..STATE_SIZE => {
        let results = results[r - 10].as_array();

        let hi = results[0];
        let lo = results[1];

        let a = lo >> 4;
        let b = hi << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi >> 36;
        let d = s_hi * 0xffff_ffffu64;
        let ret = s_lo.wrapping_add(d);
        // NOTE: this vectorizes much better than overflow flag
        let o2 = s_lo > ret || d > ret;
        let linear_result =  ret + 0xffff_ffffu64 * ((o1 as u64) + (o2 as u64));

        state[r].0 = badd(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0, linear_result);
    });
}

#[inline(always)]
fn rcs_layer_last_permute(state: [Melt; 16]) -> [Melt; 5] {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let results = mds_generated::generated_last_simd_x2(&[hi, lo]);

    let mut result = [Melt(0u64); 5];
    const_for!(r in 0..5 => {
        let results = results[r].as_array();

        let hi = results[0];
        let lo = results[1];

        let a = lo >> 4;
        let b = hi << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi >> 36;
        let d = s_hi * 0xffff_ffffu64;
        let ret = s_lo.wrapping_add(d);
        // NOTE: this vectorizes much better than overflow flag
        let o2 = s_lo > ret || d > ret;
        let linear_result = ret + 0xffff_ffffu64 * ((o1 as u64) + (o2 as u64));
        result[r].0 = badd(
            ROUND_CONSTANTS2[(NUM_ROUNDS - 1) * STATE_SIZE + r].0,
            linear_result
        );
    });

    result
}
