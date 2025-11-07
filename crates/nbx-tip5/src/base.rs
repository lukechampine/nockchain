use std::simd::prelude::*;

pub const PRIME: u64 = 18446744069414584321;
pub const PRIME_PRIME: u64 = PRIME - 2;
pub const PRIME_128: u128 = 18446744069414584321;
pub const H: u64 = 20033703337;
pub const ORDER: u64 = 2_u64.pow(32);

pub const fn based_check(a: u64) -> bool {
    a < PRIME
}

#[macro_export]
macro_rules! based {
    ( $( $x:expr ),* ) => {
      {
          $(
              debug_assert!($crate::base::based_check($x), "element must be inside the field\r");
          )*
      }
    };
}

#[inline(always)]
pub const fn badd(a: u64, b: u64) -> u64 {
    based!(a);
    based!(b);
    // NOTE: see https://docs.rs/twenty-first/latest/src/twenty_first/math/b_field_element.rs.html#686-707
    let (x1, c1) = a.overflowing_sub(PRIME - b);

    if c1 {
        x1.wrapping_add(PRIME)
    } else {
        x1
    }
}

#[inline(always)]
pub fn bneg(a: u64) -> u64 {
    based!(a);
    if a != 0 {
        PRIME - a
    } else {
        0
    }
}

#[inline(always)]
pub fn bsub(a: u64, b: u64) -> u64 {
    based!(a);
    based!(b);
    let (x1, c1) = a.overflowing_sub(b);
    x1.wrapping_sub((1 + !PRIME) * c1 as u64)
}

// Serial version of mont reduction, because this is somewhat slower than the codepath that doesn't
// block auto-vectorization
#[inline(always)]
pub const fn mont_reduction_ser(x: u128) -> u64 {
    let x1 = x as u64;
    let x2 = (x >> 64) as u64;

    let (a, e) = x1.overflowing_add(x1 << 32);
    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = x2.overflowing_sub(b);

    r.wrapping_sub((1 + !PRIME) * c as u64)
}

// ::  +montiply: computes a*b = (abr^{-1} mod p); note mul, not fmul: avoids mod p reduction!
#[inline(always)]
pub const fn montiply_ser(a: u64, b: u64) -> u64 {
    mont_reduction_ser((a as u128) * (b as u128))
}

#[inline(always)]
pub const fn mont_reduction(x: u128) -> u64 {
    let x1 = x as u64;
    let x2 = (x >> 64) as u64;

    let a = x1.wrapping_add(x1 << 32);
    let e = (x1 << 32) > a || x1 > a;
    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let r = x2.wrapping_sub(b);
    let c = b > x2;

    r.wrapping_sub((1 + !PRIME) * c as u64)
}

// ::  +montiply: computes a*b = (abr^{-1} mod p); note mul, not fmul: avoids mod p reduction!
#[inline(always)]
pub const fn montiply(a: u64, b: u64) -> u64 {
    mont_reduction((a as u128) * (b as u128))
}

#[inline(always)]
pub fn montiply_simd_x8(a: u64x8, b: u64x8) -> u64x8 {
    let a_lo = a & u64x8::splat(0xFFFFFFFF);
    let a_hi = a >> u64x8::splat(32);
    let b_lo = b & u64x8::splat(0xFFFFFFFF);
    let b_hi = b >> u64x8::splat(32);

    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;

    let p1_lo = p1 & u64x8::splat(0xFFFFFFFF);
    let p1_hi = p1 >> u64x8::splat(32);
    let p2_lo = p2 & u64x8::splat(0xFFFFFFFF);
    let p2_hi = p2 >> u64x8::splat(32);

    let mid = p1_lo + p2_lo + (p0 >> u64x8::splat(32));
    let x1 = (mid << u64x8::splat(32)) | (p0 & u64x8::splat(0xFFFFFFFF));
    let x2 = p3 + p1_hi + p2_hi + (mid >> u64x8::splat(32));

    mont_reduction_simd_x8(x1, x2)
}

#[inline(always)]
pub fn mont_reduction_simd_x8(x1: u64x8, x2: u64x8) -> u64x8 {
    let x1_shl32 = x1 << u64x8::splat(32);
    let a = x1 + x1_shl32;
    let e = a.simd_lt(x1) | a.simd_lt(x1_shl32);

    let a_shr32 = a >> u64x8::splat(32);
    let b = a - a_shr32 - e.select(u64x8::splat(1), u64x8::splat(0));

    let r = x2 - b;
    let c = x2.simd_lt(b);

    r - (u64x8::splat(1 + !PRIME) * c.select(u64x8::splat(1), u64x8::splat(0)))
}

#[inline(always)]
pub fn montiply_simd_x2(a: u64x2, b: u64x2) -> u64x2 {
    let a_lo = a & u64x2::splat(0xFFFFFFFF);
    let a_hi = a >> u64x2::splat(32);
    let b_lo = b & u64x2::splat(0xFFFFFFFF);
    let b_hi = b >> u64x2::splat(32);

    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;

    let p1_lo = p1 & u64x2::splat(0xFFFFFFFF);
    let p1_hi = p1 >> u64x2::splat(32);
    let p2_lo = p2 & u64x2::splat(0xFFFFFFFF);
    let p2_hi = p2 >> u64x2::splat(32);

    let mid = p1_lo + p2_lo + (p0 >> u64x2::splat(32));
    let x1 = (mid << u64x2::splat(32)) | (p0 & u64x2::splat(0xFFFFFFFF));
    let x2 = p3 + p1_hi + p2_hi + (mid >> u64x2::splat(32));

    mont_reduction_simd_x2(x1, x2)
}

#[inline(always)]
pub fn mont_reduction_simd_x2(x1: u64x2, x2: u64x2) -> u64x2 {
    let x1_shl32 = x1 << u64x2::splat(32);
    let a = x1 + x1_shl32;
    let e = a.simd_lt(x1) | a.simd_lt(x1_shl32);

    let a_shr32 = a >> u64x2::splat(32);
    let b = a - a_shr32 - e.select(u64x2::splat(1), u64x2::splat(0));

    let r = x2 - b;
    let c = x2.simd_lt(b);

    r - (u64x2::splat(1 + !PRIME) * c.select(u64x2::splat(1), u64x2::splat(0)))
}

// ::  +montify: transform to Montgomery space, i.e. compute x•r = xr mod p
#[inline(always)]
pub const fn montify(x: u64) -> u64 {
    // ++  r2  0xffff.fffe.0000.0001
    let r2: u64 = 0xfffffffe00000001;

    // |=  x=belt
    // ^-  melt
    // ~+
    // (montiply x r2)
    montiply(x, r2)
}

/// Reduce a 128 bit number
#[inline(always)]
pub fn reduce(n: u128) -> u64 {
    reduce_159(n as u64, (n >> 64) as u32, (n >> 96) as u64)
}

/// Reduce a 159 bit number
/// See <https://cp4space.hatsya.com/2021/09/01/an-efficient-prime-for-number-theoretic-transforms/>
/// See <https://github.com/mir-protocol/plonky2/blob/3a6d693f3ffe5aa1636e0066a4ea4885a10b5cdf/field/src/goldilocks_field.rs#L340-L356>
/// Removing both branch_hints can cause misleading changes to performance. bmul and especially
/// bpow in their micro-benchmarks will appear to be faster but higher-level stuff like bp_fft
/// will be slower. Make sure you validate your changes across the whole benchmark suite.
/// We have wrapping benchmarks (bpow(PRIME - 1, 5)) that are meant to be sensitive to the edge
/// cases but they seem to get faster anyway.
#[inline(always)]
pub fn reduce_159(low: u64, mid: u32, high: u64) -> u64 {
    let (mut low2, carry) = low.overflowing_sub(high);
    if carry {
        low2 = low2.wrapping_add(PRIME);
    }

    let mut product = (mid as u64) << 32;
    product -= product >> 32;

    let (mut result, carry) = product.overflowing_add(low2);
    if std::hint::likely(carry) {
        // This branch is likely to happen. It should compile to a use
        // branchless conditional operations. This seems counter-intuitive,
        // but we get better performance out of bpow from this branch_hint.
        result = result.wrapping_sub(PRIME);
    }

    if result >= PRIME {
        // TODO: 2025-04-26: Chris A: I'm not sure that it's actually guaranteed,
        // when I unified the two branches, it caused an error.
        // This branch is unlikely to happen. It is guaranteed not to be taken
        // if the above branch was taken. (But merging the two branches is
        // slower.)
        // TODO: 2025-04-26: Chris A: +20% improvement to roswell prove_block pow/128 vs. branch_hint
        core::hint::cold_path();
        result -= PRIME;
    }
    result
}

#[inline(always)]
pub fn bmul(a: u64, b: u64) -> u64 {
    based!(a);
    based!(b);
    reduce((a as u128) * (b as u128))
}

#[inline(always)]
pub fn bpow(mut a: u64, mut b: u64) -> u64 {
    based!(a);
    based!(b);

    let mut c: u64 = 1;
    if b == 0 {
        return c;
    }

    while b > 1 {
        if b & 1 == 0 {
            a = reduce((a as u128) * (a as u128));
            b /= 2;
        } else {
            c = reduce((c as u128) * (a as u128));
            a = reduce((a as u128) * (a as u128));
            b = (b - 1) / 2;
        }
    }
    reduce((c as u128) * (a as u128))
}

#[inline(always)]
pub fn bdiv(a: u64, b: u64) -> u64 {
    bmul(a, binv(b))
}

#[inline(always)]
pub fn binv(a: u64) -> u64 {
    based!(a);
    let y = montify(a);
    let y2 = montiply(y, montiply(y, y));
    let y3 = montiply(y, montiply(y2, y2));
    let y5 = montiply(y2, montwopow(y3, 2));
    let y10 = montiply(y5, montwopow(y5, 5));
    let y20 = montiply(y10, montwopow(y10, 10));
    let y30 = montiply(y10, montwopow(y20, 10));
    let y31 = montiply(y, montiply(y30, y30));
    let dup = montiply(montwopow(y31, 32), y31);

    mont_reduction(montiply(y, montiply(dup, dup)).into())
}

#[inline(always)]
pub fn montwopow(a: u64, b: u32) -> u64 {
    based!(a);
    // if b == 0 {
    //     return a;
    // }

    let mut res = a;
    for _ in 0..b {
        res = montiply(res, res);
    }
    res
}

#[test]
fn test_binv() {
    assert_eq!(bmul(binv(888), 888), 1);
}
