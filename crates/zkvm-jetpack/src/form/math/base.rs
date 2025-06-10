// Base field arithmetic functions.

pub const PRIME: u64 = 18446744069414584321;
pub const PRIME_PRIME: u64 = PRIME - 2;
pub const PRIME_128: u128 = 18446744069414584321;
pub const H: u64 = 20033703337;
pub const ORDER: u64 = 2_u64.pow(32);

#[derive(Debug)]
pub enum FieldError {
    OrderedRootError,
}

pub fn based_check(a: u64) -> bool {
    a < PRIME
}

#[macro_export]
macro_rules! based {
    ( $( $x:expr ),* ) => {
      {
          $(
              debug_assert!($crate::form::math::base::based_check($x), "element must be inside the field\r");
          )*
      }
    };
}

#[inline(always)]
pub fn badd(a: u64, b: u64) -> u64 {
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

/// Reduce a 128 bit number
#[inline(always)]
pub fn reduce(prod: u128) -> u64 {
    // NOTE: see https://docs.rs/risc0-core/1.2.6/src/risc0_core/field/goldilocks.rs.html#299
    let ret: u64 = prod as u64;
    // Get two high words
    let med: u32 = (prod >> 64) as u32;
    let high: u32 = (prod >> 96) as u32;
    // Subtract out high bits, add in P if underflow
    let ret = if ret >= (high as u64) {
        ret.wrapping_sub(high as u64)
    } else {
        ret.wrapping_sub(high as u64).wrapping_add(PRIME)
    };

    // Compute shifted effect of medium
    let med_shift = ((med as u64) << 32).wrapping_sub(med as u64);

    // Add in, if overflow, subtract a P
    let ret = ret.wrapping_add(med_shift);
    if ret < med_shift || ret >= PRIME {
        ret.wrapping_sub(PRIME)
    } else {
        ret
    }
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
    // Due to fermat's little theorem, a^(p-1) = 1 (mod p), so a^(p-2) = a^(-1) (mod p)
    // bpow already checks based, so we skip it here
    bpow(a, PRIME - 2)
}

#[test]
fn test_binv() {
    assert_eq!(bmul(binv(888), 888), 1);
}
