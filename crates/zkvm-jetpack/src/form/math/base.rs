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

pub const fn based_check(a: u64) -> bool {
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

#[inline(always)]
pub const fn mont_reduction(x: u128) -> u64 {
    // |=  x=melt
    // ^-  belt
    // ?>  (lth x rp)
    // assert!(x < RP);
    // =/  x1  (cut 5 [1 1] x)
    let x1 = x as u64;

    // =/  x2  (rsh 6 x)
    let x2 = (x >> 64) as u64;

    // NOTE: the rest is different. see: https://docs.rs/twenty-first/latest/src/twenty_first/math/b_field_element.rs.html#340-353
    // =/  c
    //   =/  x0  (end 5 x)
    //   (lsh 5 (add x0 x1))
    // =/  f   (rsh 6 c)
    // =/  d   (sub c (add x1 (mul f p)))
    // ?:  (gte x2 d)
    //   (sub x2 d)
    // (sub (add x2 p) d)

    let (a, e) = x1.overflowing_add(x1 << 32);
    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = x2.overflowing_sub(b);

    r.wrapping_sub((1 + !PRIME) * c as u64)
}

// ::  +montiply: computes a*b = (abr^{-1} mod p); note mul, not fmul: avoids mod p reduction!
#[inline(always)]
pub const fn montiply(a: u64, b: u64) -> u64 {
    // |:  [a=`melt`r-mod-p b=`melt`r-mod-p]
    // ^-  belt
    // ~+
    // ?>  ?&((based a) (based b))
    // FIXME: verify based
    mont_reduction((a as u128) * (b as u128))
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
#[inline(always)]
pub fn reduce_159(low: u64, mid: u32, high: u64) -> u64 {
    let (mut low2, carry) = low.overflowing_sub(high);
    if carry {
        low2 = low2.wrapping_add(PRIME);
    }

    let mut product = (mid as u64) << 32;
    product -= product >> 32;

    let (mut result, carry) = product.overflowing_add(low2);
    if carry {
        result = result.wrapping_sub(PRIME);
    }

    if result >= PRIME {
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
    // Due to fermat's little theorem, a^(p-1) = 1 (mod p), so a^(p-2) = a^(-1) (mod p)
    // bpow already checks based, so we skip it here
    bpow(a, PRIME - 2)
}

#[test]
fn test_binv() {
    assert_eq!(bmul(binv(888), 888), 1);
}
