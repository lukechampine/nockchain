#[cfg(not(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "avx512vl"
)))]
pub fn cpu_supported() -> bool {
    false
}

use std::arch::x86_64::*;
use std::sync::OnceLock;

use crate::base::*;
use crate::melt::*;
use crate::tip5::{DIGEST_LENGTH, LOOKUP_TABLE, MELT_ONE_POW_7, NUM_ROUNDS, RATE, STATE_SIZE};

static CPU_SUPPORTED: OnceLock<bool> = OnceLock::new();

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "avx512vl"
))]
pub fn cpu_supported() -> bool {
    *CPU_SUPPORTED.get_or_init(|| is_x86_feature_detected!("avx512vbmi"))
}

/// Transposed MDS matrix for efficient AVX512 multiplication
/// This is MDS_MATRIX_I64 but organized for SIMD operations
const MDS_TRANS: [[u64; 8]; 16] = [
    [61402, 1108, 28750, 33823, 7454, 43244, 53865, 12034],
    [56951, 27521, 41351, 40901, 12021, 59689, 26798, 17845],
    [17845, 61402, 1108, 28750, 33823, 7454, 43244, 53865],
    [12034, 56951, 27521, 41351, 40901, 12021, 59689, 26798],
    [26798, 17845, 61402, 1108, 28750, 33823, 7454, 43244],
    [53865, 12034, 56951, 27521, 41351, 40901, 12021, 59689],
    [59689, 26798, 17845, 61402, 1108, 28750, 33823, 7454],
    [43244, 53865, 12034, 56951, 27521, 41351, 40901, 12021],
    [12021, 59689, 26798, 17845, 61402, 1108, 28750, 33823],
    [7454, 43244, 53865, 12034, 56951, 27521, 41351, 40901],
    [40901, 12021, 59689, 26798, 17845, 61402, 1108, 28750],
    [33823, 7454, 43244, 53865, 12034, 56951, 27521, 41351],
    [41351, 40901, 12021, 59689, 26798, 17845, 61402, 1108],
    [28750, 33823, 7454, 43244, 53865, 12034, 56951, 27521],
    [27521, 41351, 40901, 12021, 59689, 26798, 17845, 61402],
    [1108, 28750, 33823, 7454, 43244, 53865, 12034, 56951],
];

/// Split ROUND_CONSTANTS2 into upper and lower 32-bit parts for efficient SIMD processing
pub const RCS_MONT_U: [u64; NUM_ROUNDS * STATE_SIZE] = const {
    let mut i = 0;
    let mut ret = [0u64; NUM_ROUNDS * STATE_SIZE];
    use crate::tip5::ROUND_CONSTANTS2;
    while i < ret.len() {
        ret[i] = (ROUND_CONSTANTS2[i].0 >> 32) & 0xffffffff;
        i += 1;
    }
    ret
};

pub const RCS_MONT_L: [u64; NUM_ROUNDS * STATE_SIZE] = const {
    let mut i = 0;
    let mut ret = [0u64; NUM_ROUNDS * STATE_SIZE];
    use crate::tip5::ROUND_CONSTANTS2;
    while i < ret.len() {
        ret[i] = ROUND_CONSTANTS2[i].0 & 0xffffffff;
        i += 1;
    }
    ret
};

pub unsafe fn permute(sponge: &mut [Melt; STATE_SIZE]) {
    let lookup_tables = load_lookup_tables();

    let mut a = _mm512_loadu_epi64(sponge.as_ptr() as *const i64);
    let mut b = _mm512_loadu_epi64(sponge.as_ptr().add(8) as *const i64);

    for round in 0..NUM_ROUNDS {
        (a, b) = sbox_layer_reg(&lookup_tables, a, b);
        (a, b) = mds_rcs_reg(a, b, round);
    }

    _mm512_storeu_epi64(sponge.as_mut_ptr() as *mut i64, a);
    _mm512_storeu_epi64(sponge.as_mut_ptr().add(8) as *mut i64, b);
}

pub unsafe fn permute_intermediate(sponge: &mut [Melt; STATE_SIZE]) {
    let lookup_tables = load_lookup_tables();

    let mut a = _mm512_loadu_epi64(sponge.as_ptr() as *const i64);
    let mut b = _mm512_loadu_epi64(sponge.as_ptr().add(8) as *const i64);

    for round in 0..(NUM_ROUNDS - 1) {
        (a, b) = sbox_layer_reg(&lookup_tables, a, b);
        (a, b) = mds_rcs_reg(a, b, round);
    }

    // Last round: only compute elements 10-15 (register b, starting at index 2)
    (a, b) = sbox_layer_reg(&lookup_tables, a, b);
    b = mds_rcs_reg_intermediate(a, b, NUM_ROUNDS - 1);

    // Only store register b since elements 0-9 will be overwritten
    _mm512_storeu_epi64(sponge.as_mut_ptr().add(8) as *mut i64, b);
}

pub unsafe fn permute_last(sponge: [Melt; STATE_SIZE]) -> [Melt; DIGEST_LENGTH] {
    let lookup_tables = load_lookup_tables();

    let mut a = _mm512_loadu_epi64(sponge.as_ptr() as *const i64);
    let mut b = _mm512_loadu_epi64(sponge.as_ptr().add(8) as *const i64);

    for round in 0..(NUM_ROUNDS - 1) {
        (a, b) = sbox_layer_reg(&lookup_tables, a, b);
        (a, b) = mds_rcs_reg(a, b, round);
    }

    // Last round: only compute first DIGEST_LENGTH elements (all in register a)
    (a, b) = sbox_layer_reg(&lookup_tables, a, b);
    a = mds_rcs_reg_last(a, b, NUM_ROUNDS - 1);

    // Extract digest directly from register
    let mut temp = [0u64; 8];
    _mm512_storeu_epi64(temp.as_mut_ptr() as *mut i64, a);

    // Copy only what we need
    let mut result = [Melt(0); DIGEST_LENGTH];
    for i in 0..DIGEST_LENGTH {
        result[i] = Melt(temp[i]);
    }
    result
}

pub unsafe fn permute_fixed(input: &[Melt; RATE]) -> [Melt; DIGEST_LENGTH] {
    let lookup_tables = load_lookup_tables();

    let (mut a, mut b) = sbox_layer_fixed_reg(&lookup_tables, input);
    (a, b) = mds_rcs_reg(a, b, 0);

    for round in 1..(NUM_ROUNDS - 1) {
        (a, b) = sbox_layer_reg(&lookup_tables, a, b);
        (a, b) = mds_rcs_reg(a, b, round);
    }

    // Last round: only compute first DIGEST_LENGTH elements (all in register a)
    (a, b) = sbox_layer_reg(&lookup_tables, a, b);
    a = mds_rcs_reg_last(a, b, NUM_ROUNDS - 1);

    // Extract digest
    let mut temp = [0u64; 8];
    _mm512_storeu_epi64(temp.as_mut_ptr() as *mut i64, a);

    // Copy only what we need
    let mut result = [Melt(0); DIGEST_LENGTH];
    for i in 0..DIGEST_LENGTH {
        result[i] = Melt(temp[i]);
    }
    result
}

pub unsafe fn permute_fixed_x2(
    input0: &[Melt; RATE],
    input1: &[Melt; RATE],
) -> ([Melt; DIGEST_LENGTH], [Melt; DIGEST_LENGTH]) {
    let lookup_tables = load_lookup_tables();

    // First round with fixed inputs
    let (mut a0, mut b0, mut a1, mut b1) = sbox_layer_fixed_x2_reg(&lookup_tables, input0, input1);
    (a0, b0, a1, b1) = mds_rcs_x2_reg(a0, b0, a1, b1, 0);

    // Middle rounds
    for round in 1..(NUM_ROUNDS - 1) {
        (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
        (a0, b0, a1, b1) = mds_rcs_x2_reg(a0, b0, a1, b1, round);
    }

    // Last round: only compute DIGEST_LENGTH elements
    (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
    a0 = mds_rcs_reg_last(a0, b0, NUM_ROUNDS - 1);
    a1 = mds_rcs_reg_last(a1, b1, NUM_ROUNDS - 1);

    // Extract both digests
    let mut temp0 = [0u64; 8];
    let mut temp1 = [0u64; 8];
    _mm512_storeu_epi64(temp0.as_mut_ptr() as *mut i64, a0);
    _mm512_storeu_epi64(temp1.as_mut_ptr() as *mut i64, a1);

    let mut result0 = [Melt(0); DIGEST_LENGTH];
    let mut result1 = [Melt(0); DIGEST_LENGTH];
    for i in 0..DIGEST_LENGTH {
        result0[i] = Melt(temp0[i]);
        result1[i] = Melt(temp1[i]);
    }

    (result0, result1)
}

pub unsafe fn permute_intermediate_x2(
    sponge0: &mut [Melt; STATE_SIZE],
    sponge1: &mut [Melt; STATE_SIZE],
) {
    let lookup_tables = load_lookup_tables();

    let mut a0 = _mm512_loadu_epi64(sponge0.as_ptr() as *const i64);
    let mut b0 = _mm512_loadu_epi64(sponge0.as_ptr().add(8) as *const i64);
    let mut a1 = _mm512_loadu_epi64(sponge1.as_ptr() as *const i64);
    let mut b1 = _mm512_loadu_epi64(sponge1.as_ptr().add(8) as *const i64);

    for round in 0..(NUM_ROUNDS - 1) {
        (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
        (a0, b0, a1, b1) = mds_rcs_x2_reg(a0, b0, a1, b1, round);
    }

    // Last round: only compute elements 10-15
    (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
    let b = mds_rcs_x2_reg_intermediate(a0, b0, a1, b1, NUM_ROUNDS - 1);

    _mm512_storeu_epi64(sponge0.as_mut_ptr().add(8) as *mut i64, b.0);
    _mm512_storeu_epi64(sponge1.as_mut_ptr().add(8) as *mut i64, b.1);
}

pub unsafe fn permute_last_x2(
    sponge0: [Melt; STATE_SIZE],
    sponge1: [Melt; STATE_SIZE],
) -> ([Melt; DIGEST_LENGTH], [Melt; DIGEST_LENGTH]) {
    let lookup_tables = load_lookup_tables();

    let mut a0 = _mm512_loadu_epi64(sponge0.as_ptr() as *const i64);
    let mut b0 = _mm512_loadu_epi64(sponge0.as_ptr().add(8) as *const i64);
    let mut a1 = _mm512_loadu_epi64(sponge1.as_ptr() as *const i64);
    let mut b1 = _mm512_loadu_epi64(sponge1.as_ptr().add(8) as *const i64);

    for round in 0..(NUM_ROUNDS - 1) {
        (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
        (a0, b0, a1, b1) = mds_rcs_x2_reg(a0, b0, a1, b1, round);
    }

    // Last round: only compute DIGEST_LENGTH elements
    (a0, b0, a1, b1) = sbox_layer_x2_reg(&lookup_tables, a0, b0, a1, b1);
    let (a0, a1) = mds_rcs_x2_reg_last(a0, b0, a1, b1, NUM_ROUNDS - 1);

    let mut temp0 = [0u64; 8];
    let mut temp1 = [0u64; 8];
    _mm512_storeu_epi64(temp0.as_mut_ptr() as *mut i64, a0);
    _mm512_storeu_epi64(temp1.as_mut_ptr() as *mut i64, a1);

    let mut result0 = [Melt(0); DIGEST_LENGTH];
    let mut result1 = [Melt(0); DIGEST_LENGTH];
    for i in 0..DIGEST_LENGTH {
        result0[i] = Melt(temp0[i]);
        result1[i] = Melt(temp1[i]);
    }
    (result0, result1)
}

/// Pre-loaded lookup tables to avoid reloading every round
struct LookupTables {
    s0: __m512i,
    s1: __m512i,
    s2: __m512i,
    s3: __m512i,
    c64s: __m512i,
}

/// Load lookup tables once
#[inline(always)]
unsafe fn load_lookup_tables() -> LookupTables {
    LookupTables {
        s0: _mm512_loadu_epi64(LOOKUP_TABLE.as_ptr() as *const i64),
        s1: _mm512_loadu_epi64(LOOKUP_TABLE.as_ptr().add(0x40) as *const i64),
        s2: _mm512_loadu_epi64(LOOKUP_TABLE.as_ptr().add(0x80) as *const i64),
        s3: _mm512_loadu_epi64(LOOKUP_TABLE.as_ptr().add(0xc0) as *const i64),
        c64s: _mm512_set1_epi8(0x40),
    }
}

#[inline(always)]
unsafe fn sbox_layer_reg(tables: &LookupTables, a: __m512i, b: __m512i) -> (__m512i, __m512i) {
    // Apply lookup table to first 4 elements (split-and-lookup)
    let mut asbox = _mm512_setzero_si512();

    // Process lookup table
    let i0 = a;
    let i1 = _mm512_sub_epi8(i0, tables.c64s);
    let i2 = _mm512_sub_epi8(i1, tables.c64s);
    let i3 = _mm512_sub_epi8(i2, tables.c64s);

    let lt0 = _mm512_cmplt_epu8_mask(i0, tables.c64s);
    let lt1 = _mm512_cmplt_epu8_mask(i1, tables.c64s);
    let lt2 = _mm512_cmplt_epu8_mask(i2, tables.c64s);
    let lt3 = _mm512_cmplt_epu8_mask(i3, tables.c64s);

    asbox = _mm512_mask_permutexvar_epi8(asbox, lt0, i0, tables.s0);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt1, i1, tables.s1);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt2, i2, tables.s2);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt3, i3, tables.s3);

    // Apply x^7 power function to remaining elements
    let a2 = square8(a);
    let b2 = square8(b);

    let a4 = square8(a2);
    let b4 = square8(b2);

    let a7 = mul8(mul8(a, a2), a4);
    let b7 = mul8(mul8(b, b2), b4);

    // Mix: first 4 elements use lookup, rest use power
    let amix = _mm512_mask_blend_epi64(0x0f, a7, asbox);

    (amix, b7)
}

#[inline(always)]
unsafe fn sbox_layer_x2_reg(
    tables: &LookupTables,
    a0: __m512i,
    b0: __m512i,
    a1: __m512i,
    b1: __m512i,
) -> (__m512i, __m512i, __m512i, __m512i) {
    // Permutation index to reverse upper half
    let idx = _mm512_set_epi64(3, 2, 1, 0, 7, 6, 5, 4);

    // Interleave first 4 elements from state0 and state1 for lookup
    let a1rev = _mm512_permutexvar_epi64(idx, a1);
    let atosub = _mm512_mask_blend_epi64(0xf0, a0, a1rev);
    let atoexp = _mm512_mask_blend_epi64(0x0f, a0, a1rev);

    // Apply lookup table to interleaved elements
    let mut asbox = _mm512_setzero_si512();

    let i0 = atosub;
    let i1 = _mm512_sub_epi8(i0, tables.c64s);
    let i2 = _mm512_sub_epi8(i1, tables.c64s);
    let i3 = _mm512_sub_epi8(i2, tables.c64s);

    let lt0 = _mm512_cmplt_epu8_mask(i0, tables.c64s);
    let lt1 = _mm512_cmplt_epu8_mask(i1, tables.c64s);
    let lt2 = _mm512_cmplt_epu8_mask(i2, tables.c64s);
    let lt3 = _mm512_cmplt_epu8_mask(i3, tables.c64s);

    asbox = _mm512_mask_permutexvar_epi8(asbox, lt0, i0, tables.s0);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt1, i1, tables.s1);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt2, i2, tables.s2);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt3, i3, tables.s3);

    let asboxrev = _mm512_permutexvar_epi64(idx, asbox);

    // Apply x^7 to remaining elements (interleaved + both b registers)
    let a2 = square8(atoexp);
    let b02 = square8(b0);
    let b12 = square8(b1);

    let a4 = square8(a2);
    let b04 = square8(b02);
    let b14 = square8(b12);

    let a7 = mul8(mul8(atoexp, a2), a4);
    let b07 = mul8(mul8(b0, b02), b04);
    let b17 = mul8(mul8(b1, b12), b14);

    // Separate results back to individual states
    let a7rev = _mm512_permutexvar_epi64(idx, a7);
    let out0 = _mm512_mask_blend_epi64(0x0f, a7, asbox);
    let out1 = _mm512_mask_blend_epi64(0x0f, a7rev, asboxrev);

    (out0, b07, out1, b17)
}

#[inline(always)]
unsafe fn sbox_layer_fixed_x2_reg(
    tables: &LookupTables,
    input0: &[Melt; RATE],
    input1: &[Melt; RATE],
) -> (__m512i, __m512i, __m512i, __m512i) {
    // Pre-computed pow(7) for padding
    let b_base = _mm512_set_epi64(
        MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64,
        MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64, 0, 0,
    );

    // Load inputs
    let a0 = _mm512_loadu_epi64(input0.as_ptr() as *const i64);
    let a1 = _mm512_loadu_epi64(input1.as_ptr() as *const i64);

    // Interleave first 4 elements for lookup
    let idx = _mm512_set_epi64(3, 2, 1, 0, 7, 6, 5, 4);
    let a1rev = _mm512_permutexvar_epi64(idx, a1);
    let atosub = _mm512_mask_blend_epi64(0xf0, a0, a1rev);
    let atoexp = _mm512_mask_blend_epi64(0x0f, a0, a1rev);

    // Apply lookup table
    let mut asbox = _mm512_setzero_si512();

    let i0 = atosub;
    let i1 = _mm512_sub_epi8(i0, tables.c64s);
    let i2 = _mm512_sub_epi8(i1, tables.c64s);
    let i3 = _mm512_sub_epi8(i2, tables.c64s);

    let lt0 = _mm512_cmplt_epu8_mask(i0, tables.c64s);
    let lt1 = _mm512_cmplt_epu8_mask(i1, tables.c64s);
    let lt2 = _mm512_cmplt_epu8_mask(i2, tables.c64s);
    let lt3 = _mm512_cmplt_epu8_mask(i3, tables.c64s);

    asbox = _mm512_mask_permutexvar_epi8(asbox, lt0, i0, tables.s0);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt1, i1, tables.s1);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt2, i2, tables.s2);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt3, i3, tables.s3);

    let asboxrev = _mm512_permutexvar_epi64(idx, asbox);

    // Apply x^7 to elements 4-7
    let a2 = square8(atoexp);
    let a4 = square8(a2);
    let a7 = mul8(mul8(atoexp, a2), a4);
    let a7rev = _mm512_permutexvar_epi64(idx, a7);

    // Combine results
    let out0 = _mm512_mask_blend_epi64(0x0f, a7, asbox);
    let out1 = _mm512_mask_blend_epi64(0x0f, a7rev, asboxrev);

    // Compute elements 8-9 for both states
    let mut b0_vals = [0u64; 8];
    let mut b1_vals = [0u64; 8];
    _mm512_storeu_epi64(b0_vals.as_mut_ptr() as *mut i64, b_base);
    _mm512_storeu_epi64(b1_vals.as_mut_ptr() as *mut i64, b_base);

    for j in 0..2 {
        let s0 = input0[8 + j].0;
        let s02 = montiply_ser(s0, s0);
        let s04 = montiply_ser(s02, s02);
        b0_vals[j] = montiply_ser(montiply_ser(s0, s02), s04);

        let s1 = input1[8 + j].0;
        let s12 = montiply_ser(s1, s1);
        let s14 = montiply_ser(s12, s12);
        b1_vals[j] = montiply_ser(montiply_ser(s1, s12), s14);
    }

    let b0_final = _mm512_loadu_epi64(b0_vals.as_ptr() as *const i64);
    let b1_final = _mm512_loadu_epi64(b1_vals.as_ptr() as *const i64);

    (out0, b0_final, out1, b1_final)
}

#[inline(always)]
unsafe fn mds_rcs_x2_reg_intermediate(
    a0: __m512i,
    b0: __m512i,
    a1: __m512i,
    b1: __m512i,
    round_index: usize,
) -> (__m512i, __m512i) {
    let rcs_offset = round_index * 16;

    // Only initialize accumulator for register b (elements 10-15) for both states
    let rc1_lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset + 8) as *const i64);
    let rc1_hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset + 8) as *const i64);

    let mut r0_1lo = rc1_lo;
    let mut r0_1hi = rc1_hi;
    let mut r1_1lo = rc1_lo;
    let mut r1_1hi = rc1_hi;

    // Extract state values
    #[repr(C, align(64))]
    struct StateVals {
        a0: [u32; 16],
        b0: [u32; 16],
        a1: [u32; 16],
        b1: [u32; 16],
    }

    let mut vals = StateVals {
        a0: [0u32; 16],
        b0: [0u32; 16],
        a1: [0u32; 16],
        b1: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a0.as_mut_ptr() as *mut i32, a0);
    _mm512_storeu_epi32(vals.b0.as_mut_ptr() as *mut i32, b0);
    _mm512_storeu_epi32(vals.a1.as_mut_ptr() as *mut i32, a1);
    _mm512_storeu_epi32(vals.b1.as_mut_ptr() as *mut i32, b1);

    // Only compute columns for outputs 8-15 (register b)
    for i in 0..8 {
        let c1 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64);
        let c0 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64);

        // State 0 broadcasts
        let d0_0lo = _mm512_set1_epi64(vals.a0[2 * i] as i64);
        let d0_0hi = _mm512_set1_epi64(vals.a0[2 * i + 1] as i64);
        let e0_0lo = _mm512_set1_epi64(vals.b0[2 * i] as i64);
        let e0_0hi = _mm512_set1_epi64(vals.b0[2 * i + 1] as i64);

        // State 1 broadcasts
        let d1_0lo = _mm512_set1_epi64(vals.a1[2 * i] as i64);
        let d1_0hi = _mm512_set1_epi64(vals.a1[2 * i + 1] as i64);
        let e1_0lo = _mm512_set1_epi64(vals.b1[2 * i] as i64);
        let e1_0hi = _mm512_set1_epi64(vals.b1[2 * i + 1] as i64);

        // State 0 products (only for b output)
        let prod0_2 = _mm512_mul_epu32(c1, d0_0lo);
        let prod0_3 = _mm512_mul_epu32(c1, d0_0hi);
        let prod0_6 = _mm512_mul_epu32(c0, e0_0lo);
        let prod0_7 = _mm512_mul_epu32(c0, e0_0hi);

        // State 1 products (only for b output)
        let prod1_2 = _mm512_mul_epu32(c1, d1_0lo);
        let prod1_3 = _mm512_mul_epu32(c1, d1_0hi);
        let prod1_6 = _mm512_mul_epu32(c0, e1_0lo);
        let prod1_7 = _mm512_mul_epu32(c0, e1_0hi);

        // Accumulate
        r0_1lo = _mm512_add_epi64(r0_1lo, prod0_2);
        r0_1hi = _mm512_add_epi64(r0_1hi, prod0_3);
        r0_1lo = _mm512_add_epi64(r0_1lo, prod0_6);
        r0_1hi = _mm512_add_epi64(r0_1hi, prod0_7);

        r1_1lo = _mm512_add_epi64(r1_1lo, prod1_2);
        r1_1hi = _mm512_add_epi64(r1_1hi, prod1_3);
        r1_1lo = _mm512_add_epi64(r1_1lo, prod1_6);
        r1_1hi = _mm512_add_epi64(r1_1hi, prod1_7);
    }

    (reduce2x32(r0_1lo, r0_1hi), reduce2x32(r1_1lo, r1_1hi))
}

#[inline(always)]
unsafe fn mds_rcs_x2_reg_last(
    a0: __m512i,
    b0: __m512i,
    a1: __m512i,
    b1: __m512i,
    round_index: usize,
) -> (__m512i, __m512i) {
    let rcs_offset = round_index * 16;

    // Only initialize accumulator for register a (elements 0-7)
    let rc_lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset) as *const i64);
    let rc_hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset) as *const i64);

    let mut r0_0lo = rc_lo;
    let mut r0_0hi = rc_hi;
    let mut r1_0lo = rc_lo;
    let mut r1_0hi = rc_hi;

    // Extract state values
    #[repr(C, align(64))]
    struct StateVals {
        a0: [u32; 16],
        b0: [u32; 16],
        a1: [u32; 16],
        b1: [u32; 16],
    }

    let mut vals = StateVals {
        a0: [0u32; 16],
        b0: [0u32; 16],
        a1: [0u32; 16],
        b1: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a0.as_mut_ptr() as *mut i32, a0);
    _mm512_storeu_epi32(vals.b0.as_mut_ptr() as *mut i32, b0);
    _mm512_storeu_epi32(vals.a1.as_mut_ptr() as *mut i32, a1);
    _mm512_storeu_epi32(vals.b1.as_mut_ptr() as *mut i32, b1);

    // Only compute columns for outputs 0-7 (register a)
    for i in 0..8 {
        let c0 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64);
        let c1 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64);

        // State 0 broadcasts
        let d0_0lo = _mm512_set1_epi64(vals.a0[2 * i] as i64);
        let d0_0hi = _mm512_set1_epi64(vals.a0[2 * i + 1] as i64);
        let e0_0lo = _mm512_set1_epi64(vals.b0[2 * i] as i64);
        let e0_0hi = _mm512_set1_epi64(vals.b0[2 * i + 1] as i64);

        // State 1 broadcasts
        let d1_0lo = _mm512_set1_epi64(vals.a1[2 * i] as i64);
        let d1_0hi = _mm512_set1_epi64(vals.a1[2 * i + 1] as i64);
        let e1_0lo = _mm512_set1_epi64(vals.b1[2 * i] as i64);
        let e1_0hi = _mm512_set1_epi64(vals.b1[2 * i + 1] as i64);

        // State 0 products (only for a output)
        let prod0_0 = _mm512_mul_epu32(c0, d0_0lo);
        let prod0_1 = _mm512_mul_epu32(c0, d0_0hi);
        let prod0_4 = _mm512_mul_epu32(c1, e0_0lo);
        let prod0_5 = _mm512_mul_epu32(c1, e0_0hi);

        // State 1 products (only for a output)
        let prod1_0 = _mm512_mul_epu32(c0, d1_0lo);
        let prod1_1 = _mm512_mul_epu32(c0, d1_0hi);
        let prod1_4 = _mm512_mul_epu32(c1, e1_0lo);
        let prod1_5 = _mm512_mul_epu32(c1, e1_0hi);

        // Accumulate
        r0_0lo = _mm512_add_epi64(r0_0lo, prod0_0);
        r0_0hi = _mm512_add_epi64(r0_0hi, prod0_1);
        r0_0lo = _mm512_add_epi64(r0_0lo, prod0_4);
        r0_0hi = _mm512_add_epi64(r0_0hi, prod0_5);

        r1_0lo = _mm512_add_epi64(r1_0lo, prod1_0);
        r1_0hi = _mm512_add_epi64(r1_0hi, prod1_1);
        r1_0lo = _mm512_add_epi64(r1_0lo, prod1_4);
        r1_0hi = _mm512_add_epi64(r1_0hi, prod1_5);
    }

    (reduce2x32(r0_0lo, r0_0hi), reduce2x32(r1_0lo, r1_0hi))
}

/// S-box layer for fixed input (knows padding values)
#[inline(always)]
unsafe fn sbox_layer_fixed_reg(tables: &LookupTables, input: &[Melt; RATE]) -> (__m512i, __m512i) {
    // Because RATE is only 10, there are 6 elements remaining that are set to Melt::one(). The
    //  pow(7) of these values are pre-computed.
    let b = _mm512_set_epi64(
        MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64,
        MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64, MELT_ONE_POW_7.0 as i64,
        0, // Will be computed
        0, // Will be computed
    );

    // Load input
    let a = _mm512_loadu_epi64(input.as_ptr() as *const i64);

    // Apply lookup table to first 4 elements
    let mut asbox = _mm512_setzero_si512();

    let i0 = a;
    let i1 = _mm512_sub_epi8(i0, tables.c64s);
    let i2 = _mm512_sub_epi8(i1, tables.c64s);
    let i3 = _mm512_sub_epi8(i2, tables.c64s);

    let lt0 = _mm512_cmplt_epu8_mask(i0, tables.c64s);
    let lt1 = _mm512_cmplt_epu8_mask(i1, tables.c64s);
    let lt2 = _mm512_cmplt_epu8_mask(i2, tables.c64s);
    let lt3 = _mm512_cmplt_epu8_mask(i3, tables.c64s);

    asbox = _mm512_mask_permutexvar_epi8(asbox, lt0, i0, tables.s0);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt1, i1, tables.s1);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt2, i2, tables.s2);
    asbox = _mm512_mask_permutexvar_epi8(asbox, lt3, i3, tables.s3);

    // Apply x^7 to elements 4-7
    let a2 = square8(a);
    let a4 = square8(a2);
    let a7 = mul8(mul8(a, a2), a4);

    // Mix results
    let amix = _mm512_mask_blend_epi64(0x0f, a7, asbox);

    // Compute remaining 2 elements (8-9) using scalar
    let mut b_vals = [0u64; 8];
    _mm512_storeu_epi64(b_vals.as_mut_ptr() as *mut i64, b);

    for j in 0..2 {
        let s1 = input[8 + j].0;
        let s2 = montiply_ser(s1, s1);
        let s4 = montiply_ser(s2, s2);
        b_vals[j] = montiply_ser(montiply_ser(s1, s2), s4);
    }

    let b_final = _mm512_loadu_epi64(b_vals.as_ptr() as *const i64);

    (amix, b_final)
}

/// Combined MDS matrix multiplication and round constant addition
/// Uses standard AVX-512 multiplication instead of IFMA to avoid frequency throttling
#[inline(always)]
unsafe fn mds_rcs_reg(a: __m512i, b: __m512i, round_index: usize) -> (__m512i, __m512i) {
    let rcs_offset = round_index * 16;

    // Initialize accumulators with round constants
    let mut r0lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset) as *const i64);
    let mut r1lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset + 8) as *const i64);
    let mut r0hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset) as *const i64);
    let mut r1hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset + 8) as *const i64);

    // Extract state values to array
    #[repr(C, align(64))]
    struct StateVals {
        a: [u32; 16],
        b: [u32; 16],
    }

    let mut vals = StateVals {
        a: [0u32; 16],
        b: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a.as_mut_ptr() as *mut i32, a);
    _mm512_storeu_epi32(vals.b.as_mut_ptr() as *mut i32, b);

    // Matrix multiplication using standard multiply-add (no IFMA)
    for i in 0..8 {
        let c0 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64);
        let c1 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64);

        // Broadcast individual 32-bit elements across lanes
        let d0lo = _mm512_set1_epi64(vals.a[2 * i] as i64);
        let d0hi = _mm512_set1_epi64(vals.a[2 * i + 1] as i64);
        let e0lo = _mm512_set1_epi64(vals.b[2 * i] as i64);
        let e0hi = _mm512_set1_epi64(vals.b[2 * i + 1] as i64);

        // Standard multiply-add operations (no IFMA)
        // mul_epu32 multiplies lower 32 bits of each 64-bit lane
        let prod0 = _mm512_mul_epu32(c0, d0lo);
        let prod1 = _mm512_mul_epu32(c0, d0hi);
        let prod2 = _mm512_mul_epu32(c1, d0lo);
        let prod3 = _mm512_mul_epu32(c1, d0hi);
        let prod4 = _mm512_mul_epu32(c1, e0lo);
        let prod5 = _mm512_mul_epu32(c1, e0hi);
        let prod6 = _mm512_mul_epu32(c0, e0lo);
        let prod7 = _mm512_mul_epu32(c0, e0hi);

        // Add products to accumulators
        r0lo = _mm512_add_epi64(r0lo, prod0);
        r0hi = _mm512_add_epi64(r0hi, prod1);
        r1lo = _mm512_add_epi64(r1lo, prod2);
        r1hi = _mm512_add_epi64(r1hi, prod3);
        r0lo = _mm512_add_epi64(r0lo, prod4);
        r0hi = _mm512_add_epi64(r0hi, prod5);
        r1lo = _mm512_add_epi64(r1lo, prod6);
        r1hi = _mm512_add_epi64(r1hi, prod7);
    }

    // Reduce and return results
    (reduce2x32(r0lo, r0hi), reduce2x32(r1lo, r1hi))
}

/// MDS + round constants for intermediate permutation (only compute elements 10-15)
#[inline(always)]
unsafe fn mds_rcs_reg_intermediate(a: __m512i, b: __m512i, round_index: usize) -> __m512i {
    let rcs_offset = round_index * 16;

    // Only initialize accumulator for register b (elements 10-15)
    let mut r1lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset + 8) as *const i64);
    let mut r1hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset + 8) as *const i64);

    // Extract state values
    #[repr(C, align(64))]
    struct StateVals {
        a: [u32; 16],
        b: [u32; 16],
    }

    let mut vals = StateVals {
        a: [0u32; 16],
        b: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a.as_mut_ptr() as *mut i32, a);
    _mm512_storeu_epi32(vals.b.as_mut_ptr() as *mut i32, b);

    // Only compute columns for outputs 8-15 (register b)
    for i in 0..8 {
        let c1 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64);

        let d0lo = _mm512_set1_epi64(vals.a[2 * i] as i64);
        let d0hi = _mm512_set1_epi64(vals.a[2 * i + 1] as i64);
        let e0lo = _mm512_set1_epi64(vals.b[2 * i] as i64);
        let e0hi = _mm512_set1_epi64(vals.b[2 * i + 1] as i64);

        let prod2 = _mm512_mul_epu32(c1, d0lo);
        let prod3 = _mm512_mul_epu32(c1, d0hi);
        let prod6 = _mm512_mul_epu32(
            _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64),
            e0lo,
        );
        let prod7 = _mm512_mul_epu32(
            _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64),
            e0hi,
        );

        r1lo = _mm512_add_epi64(r1lo, prod2);
        r1hi = _mm512_add_epi64(r1hi, prod3);
        r1lo = _mm512_add_epi64(r1lo, prod6);
        r1hi = _mm512_add_epi64(r1hi, prod7);
    }

    reduce2x32(r1lo, r1hi)
}

#[inline(always)]
unsafe fn mds_rcs_x2_reg(
    a0: __m512i,
    b0: __m512i,
    a1: __m512i,
    b1: __m512i,
    round_index: usize,
) -> (__m512i, __m512i, __m512i, __m512i) {
    let rcs_offset = round_index * 16;

    // Initialize accumulators for both states with round constants
    let rc_lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset) as *const i64);
    let rc_hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset) as *const i64);
    let rc1_lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset + 8) as *const i64);
    let rc1_hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset + 8) as *const i64);

    let mut r0_0lo = rc_lo;
    let mut r0_0hi = rc_hi;
    let mut r0_1lo = rc1_lo;
    let mut r0_1hi = rc1_hi;

    let mut r1_0lo = rc_lo;
    let mut r1_0hi = rc_hi;
    let mut r1_1lo = rc1_lo;
    let mut r1_1hi = rc1_hi;

    // Extract state values for both states
    #[repr(C, align(64))]
    struct StateVals {
        a0: [u32; 16],
        b0: [u32; 16],
        a1: [u32; 16],
        b1: [u32; 16],
    }

    let mut vals = StateVals {
        a0: [0u32; 16],
        b0: [0u32; 16],
        a1: [0u32; 16],
        b1: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a0.as_mut_ptr() as *mut i32, a0);
    _mm512_storeu_epi32(vals.b0.as_mut_ptr() as *mut i32, b0);
    _mm512_storeu_epi32(vals.a1.as_mut_ptr() as *mut i32, a1);
    _mm512_storeu_epi32(vals.b1.as_mut_ptr() as *mut i32, b1);

    // Matrix multiplication for both states
    for i in 0..8 {
        // Load MDS coefficients once for both states
        let c0 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64);
        let c1 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64);

        // State 0 broadcasts
        let d0_0lo = _mm512_set1_epi64(vals.a0[2 * i] as i64);
        let d0_0hi = _mm512_set1_epi64(vals.a0[2 * i + 1] as i64);
        let e0_0lo = _mm512_set1_epi64(vals.b0[2 * i] as i64);
        let e0_0hi = _mm512_set1_epi64(vals.b0[2 * i + 1] as i64);

        // State 1 broadcasts
        let d1_0lo = _mm512_set1_epi64(vals.a1[2 * i] as i64);
        let d1_0hi = _mm512_set1_epi64(vals.a1[2 * i + 1] as i64);
        let e1_0lo = _mm512_set1_epi64(vals.b1[2 * i] as i64);
        let e1_0hi = _mm512_set1_epi64(vals.b1[2 * i + 1] as i64);

        // State 0 products
        let prod0_0 = _mm512_mul_epu32(c0, d0_0lo);
        let prod0_1 = _mm512_mul_epu32(c0, d0_0hi);
        let prod0_2 = _mm512_mul_epu32(c1, d0_0lo);
        let prod0_3 = _mm512_mul_epu32(c1, d0_0hi);
        let prod0_4 = _mm512_mul_epu32(c1, e0_0lo);
        let prod0_5 = _mm512_mul_epu32(c1, e0_0hi);
        let prod0_6 = _mm512_mul_epu32(c0, e0_0lo);
        let prod0_7 = _mm512_mul_epu32(c0, e0_0hi);

        // State 1 products
        let prod1_0 = _mm512_mul_epu32(c0, d1_0lo);
        let prod1_1 = _mm512_mul_epu32(c0, d1_0hi);
        let prod1_2 = _mm512_mul_epu32(c1, d1_0lo);
        let prod1_3 = _mm512_mul_epu32(c1, d1_0hi);
        let prod1_4 = _mm512_mul_epu32(c1, e1_0lo);
        let prod1_5 = _mm512_mul_epu32(c1, e1_0hi);
        let prod1_6 = _mm512_mul_epu32(c0, e1_0lo);
        let prod1_7 = _mm512_mul_epu32(c0, e1_0hi);

        // Accumulate state 0
        r0_0lo = _mm512_add_epi64(r0_0lo, prod0_0);
        r0_0hi = _mm512_add_epi64(r0_0hi, prod0_1);
        r0_1lo = _mm512_add_epi64(r0_1lo, prod0_2);
        r0_1hi = _mm512_add_epi64(r0_1hi, prod0_3);
        r0_0lo = _mm512_add_epi64(r0_0lo, prod0_4);
        r0_0hi = _mm512_add_epi64(r0_0hi, prod0_5);
        r0_1lo = _mm512_add_epi64(r0_1lo, prod0_6);
        r0_1hi = _mm512_add_epi64(r0_1hi, prod0_7);

        // Accumulate state 1
        r1_0lo = _mm512_add_epi64(r1_0lo, prod1_0);
        r1_0hi = _mm512_add_epi64(r1_0hi, prod1_1);
        r1_1lo = _mm512_add_epi64(r1_1lo, prod1_2);
        r1_1hi = _mm512_add_epi64(r1_1hi, prod1_3);
        r1_0lo = _mm512_add_epi64(r1_0lo, prod1_4);
        r1_0hi = _mm512_add_epi64(r1_0hi, prod1_5);
        r1_1lo = _mm512_add_epi64(r1_1lo, prod1_6);
        r1_1hi = _mm512_add_epi64(r1_1hi, prod1_7);
    }

    // Reduce and return all results
    (
        reduce2x32(r0_0lo, r0_0hi),
        reduce2x32(r0_1lo, r0_1hi),
        reduce2x32(r1_0lo, r1_0hi),
        reduce2x32(r1_1lo, r1_1hi),
    )
}

/// MDS + round constants for last permutation (only compute elements 0-4 for digest)
#[inline(always)]
unsafe fn mds_rcs_reg_last(a: __m512i, b: __m512i, round_index: usize) -> __m512i {
    let rcs_offset = round_index * 16;

    // Only initialize accumulator for register a (elements 0-7, but we only need 0-4)
    let mut r0lo = _mm512_loadu_epi64(RCS_MONT_L.as_ptr().add(rcs_offset) as *const i64);
    let mut r0hi = _mm512_loadu_epi64(RCS_MONT_U.as_ptr().add(rcs_offset) as *const i64);

    // Extract state values
    #[repr(C, align(64))]
    struct StateVals {
        a: [u32; 16],
        b: [u32; 16],
    }

    let mut vals = StateVals {
        a: [0u32; 16],
        b: [0u32; 16],
    };

    _mm512_storeu_epi32(vals.a.as_mut_ptr() as *mut i32, a);
    _mm512_storeu_epi32(vals.b.as_mut_ptr() as *mut i32, b);

    // Only compute columns for outputs 0-7 (register a)
    for i in 0..8 {
        let c0 = _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i) as *const i64);

        let d0lo = _mm512_set1_epi64(vals.a[2 * i] as i64);
        let d0hi = _mm512_set1_epi64(vals.a[2 * i + 1] as i64);
        let e0lo = _mm512_set1_epi64(vals.b[2 * i] as i64);
        let e0hi = _mm512_set1_epi64(vals.b[2 * i + 1] as i64);

        let prod0 = _mm512_mul_epu32(c0, d0lo);
        let prod1 = _mm512_mul_epu32(c0, d0hi);
        let prod4 = _mm512_mul_epu32(
            _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64),
            e0lo,
        );
        let prod5 = _mm512_mul_epu32(
            _mm512_loadu_epi64(MDS_TRANS.as_ptr().add(2 * i + 1) as *const i64),
            e0hi,
        );

        r0lo = _mm512_add_epi64(r0lo, prod0);
        r0hi = _mm512_add_epi64(r0hi, prod1);
        r0lo = _mm512_add_epi64(r0lo, prod4);
        r0hi = _mm512_add_epi64(r0hi, prod5);
    }

    reduce2x32(r0lo, r0hi)
}

/// Reduce two 32-bit limbs modulo P
/// Each argument must be at most 53 bits wide
#[inline(always)]
unsafe fn reduce2x32(lo: __m512i, hi: __m512i) -> __m512i {
    let u32_max = _mm512_set1_epi64(u32::MAX as i64);

    let x0 = _mm512_and_epi64(lo, u32_max);

    let lo_shr32 = _mm512_srli_epi64(lo, 32);
    let x_tmp = _mm512_add_epi64(lo_shr32, hi);
    let x1 = _mm512_and_epi64(x_tmp, u32_max);
    let x2 = _mm512_srli_epi64(x_tmp, 32);

    // r = ((x₁ + x₂) << 32) + x0 - x2
    let x1_plus_x2 = _mm512_add_epi64(x1, x2);
    let x1_plus_x2_shl32 = _mm512_slli_epi64(x1_plus_x2, 32);
    let x1_plus_x2_shl32_plus_x0 = _mm512_add_epi64(x1_plus_x2_shl32, x0);
    let r = _mm512_sub_epi64(x1_plus_x2_shl32_plus_x0, x2);

    // Subtract P if needed
    let p = _mm512_set1_epi64(PRIME_128 as i64);
    let r_ge_p = _mm512_cmpge_epu64_mask(r, p);
    let x1_p_x2_gt_u32 = _mm512_cmpgt_epu64_mask(x1_plus_x2, u32_max);
    let ov_mask = r_ge_p | x1_p_x2_gt_u32;
    _mm512_mask_sub_epi64(r, ov_mask, r, p)
}

/// Multiply 8 field elements using scalar Montgomery multiplication
#[inline(always)]
unsafe fn mul8(x: __m512i, y: __m512i) -> __m512i {
    let mut x_vals = [0u64; 8];
    let mut y_vals = [0u64; 8];
    let mut result = [0u64; 8];

    _mm512_storeu_epi64(x_vals.as_mut_ptr() as *mut i64, x);
    _mm512_storeu_epi64(y_vals.as_mut_ptr() as *mut i64, y);

    for i in 0..8 {
        result[i] = montiply_ser(x_vals[i], y_vals[i]);
    }

    _mm512_loadu_epi64(result.as_ptr() as *const i64)
}

/// Square 8 field elements using scalar Montgomery multiplication
/// Extracts each element, uses scalar montiply_ser, then recombines
#[inline(always)]
unsafe fn square8(x: __m512i) -> __m512i {
    let mut x_vals = [0u64; 8];
    let mut result = [0u64; 8];

    _mm512_storeu_epi64(x_vals.as_mut_ptr() as *mut i64, x);

    for i in 0..8 {
        result[i] = montiply_ser(x_vals[i], x_vals[i]);
    }

    _mm512_loadu_epi64(result.as_ptr() as *const i64)
}
