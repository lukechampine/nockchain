use std::ops::{Add, Div, Mul, Neg, Sub};

use nockvm::noun::Noun;
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

impl TryFrom<Noun> for Melt {
    type Error = ();

    #[inline(always)]
    fn try_from(n: Noun) -> std::result::Result<Self, Self::Error> {
        if !n.is_atom() {
            Err(())
        } else {
            Ok(Melt::from_u64(n.as_atom()?.as_u64()?))
        }
    }
}
