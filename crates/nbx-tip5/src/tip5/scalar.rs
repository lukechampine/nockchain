use const_for::const_for;

use crate::base::{badd, montiply_ser};
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
const fn sbox_layer(state: &[Melt; STATE_SIZE]) -> [Melt; STATE_SIZE] {
    let mut res: [Melt; STATE_SIZE] = [Melt(0); STATE_SIZE];

    const_for!(i in 0..NUM_SPLIT_AND_LOOKUP => {
        let mut bytes = state[i].0.to_le_bytes();
        const_for!(i in 0..8 => {
            bytes[i] = LOOKUP_TABLE[bytes[i] as usize];
        });
        res[i] = Melt(u64::from_le_bytes(bytes));
    });

    const_for!(j in NUM_SPLIT_AND_LOOKUP..STATE_SIZE => {
        let s1 = state[j].0;
        let s2 = montiply_ser(s1, s1);
        let s4 = montiply_ser(s2, s2);
        res[j].0 = montiply_ser(montiply_ser(s1, s2), s4);
    });

    res
}

#[inline(always)]
const fn sbox_layer_fixed(input: &[Melt; 10]) -> [Melt; STATE_SIZE] {
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
    const_for!(j in NUM_SPLIT_AND_LOOKUP..10 => {
        let s1 = input[j].0;
        let s2 = montiply_ser(s1, s1);
        let s4 = montiply_ser(s2, s2);
        res[j].0 = montiply_ser(montiply_ser(s1, s2), s4);
    });

    res
}

#[inline(always)]
const fn rcs_layer(state: &mut [Melt; 16], round_num: usize) {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let lo = mds_generated::generated(&lo);
    let hi = mds_generated::generated(&hi);

    const_for!(r in 0..STATE_SIZE => {
        let a = lo[r] >> 4;
        let b = hi[r] << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi[r] >> 36;
        let d = s_hi * 0xffff_ffffu64;
        let ret = s_lo.wrapping_add(d);
        // NOTE: this vectorizes much better than overflow flag
        let o2 = s_lo > ret || d > ret;
        let linear_result = ret + 0xffff_ffffu64 * ((o1 as u64) + (o2 as u64));

        state[r].0 = badd(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0, linear_result);
    });
}

#[inline(always)]
const fn rcs_layer_intermediate_permute(state: &mut [Melt; 16], round_num: usize) {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let lo = mds_generated::generated_intermediate(&lo);
    let hi = mds_generated::generated_intermediate(&hi);

    // Because the first 10 elements will be overwritten in the next step, there is no need to compute them
    const_for!(r in 10..STATE_SIZE => {
        let a = lo[r - 10] >> 4;
        let b = hi[r - 10] << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi[r - 10] >> 36;
        let d = s_hi * 0xffff_ffffu64;
        let ret = s_lo.wrapping_add(d);
        // NOTE: this vectorizes much better than overflow flag
        let o2 = s_lo > ret || d > ret;
        let linear_result =  ret + 0xffff_ffffu64 * ((o1 as u64) + (o2 as u64));

        state[r].0 = badd(ROUND_CONSTANTS2[round_num * STATE_SIZE + r].0, linear_result);
    });
}

#[inline(always)]
const fn rcs_layer_last_permute(state: [Melt; 16]) -> [Melt; 5] {
    let mut lo: [u32; STATE_SIZE] = [0; STATE_SIZE];
    let mut hi: [u32; STATE_SIZE] = [0; STATE_SIZE];

    const_for!(i in 0..STATE_SIZE => {
        let b = state[i].0;
        hi[i] = (b >> 32) as u32;
        lo[i] = b as u32;
    });

    let lo = mds_generated::generated_last(&lo);
    let hi = mds_generated::generated_last(&hi);

    let mut result = [Melt(0u64); 5];
    const_for!(r in 0..5 => {
        let a = lo[r] >> 4;
        let b = hi[r] << 28;
        let s_lo = a.wrapping_add(b);
        // NOTE: this vectorizes much better than overflow flag
        let o1 = a > s_lo || b > s_lo;
        let s_hi = hi[r] >> 36;
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
