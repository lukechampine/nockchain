use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub};

use noun_serde::{NounDecode, NounEncode};
use num_traits::Pow;

use crate::base::*;

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    bytemuck::Zeroable,
    bytemuck::Pod,
)]
#[repr(transparent)]
pub struct Melt(pub u64);

unsafe impl Send for Melt {}
unsafe impl Sync for Melt {}

impl Melt {
    pub fn inv(self) -> Self {
        self.pow(PRIME - 2)
    }

    pub const fn from_u64(v: u64) -> Self {
        Self(montify(v))
    }

    pub const fn pow(self, exp: u64) -> Self {
        let mut acc = Melt::from_u64(1).0;
        let bit_length = u64::BITS - exp.leading_zeros();
        let mut i = 0;
        while i < bit_length {
            acc = montiply(acc, acc);
            if exp & (1 << (bit_length - 1 - i)) != 0 {
                acc = montiply(acc, self.0);
            }
            i += 1;
        }

        Melt(acc)
    }
}

impl Add for Melt {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        let a = self.0;
        let b = rhs.0;
        Melt(badd(a, b))
    }
}

impl AddAssign for Melt {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Melt {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(bsub(self.0, rhs.0))
    }
}

impl Neg for Melt {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        Self(bneg(self.0))
    }
}

impl Mul for Melt {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self.0;
        let b = rhs.0;
        Melt(montiply(a, b))
    }
}

impl MulAssign for Melt {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Pow<u64> for Melt {
    type Output = Self;

    #[inline(always)]
    fn pow(self, exp: u64) -> Self::Output {
        Melt::pow(self, exp)
    }
}

impl Pow<usize> for Melt {
    type Output = Self;

    #[inline(always)]
    fn pow(self, rhs: usize) -> Self::Output {
        self.pow(rhs as u64).into()
    }
}

impl Div for Melt {
    type Output = Self;

    #[inline(always)]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self::Output {
        rhs.inv() * self
    }
}

impl NounEncode for Melt {
    fn to_noun<A: nockvm::noun::NounAllocator>(&self, allocator: &mut A) -> nockvm::noun::Noun {
        nockvm::noun::Atom::new(allocator, self.0).as_noun()
    }
}

impl NounDecode for Melt {
    fn from_noun(noun: &nockvm::noun::Noun) -> Result<Self, noun_serde::NounDecodeError> {
        let atom = noun
            .as_atom()
            .map_err(|_| noun_serde::NounDecodeError::ExpectedAtom)?;
        let value = atom
            .as_u64()
            .map_err(|_| noun_serde::NounDecodeError::Custom("Melt value too large".to_string()))?;
        if !based_check(value) {
            return Err(noun_serde::NounDecodeError::Custom(
                "Melt value not based".to_string(),
            ));
        }
        Ok(Melt(value))
    }
}
