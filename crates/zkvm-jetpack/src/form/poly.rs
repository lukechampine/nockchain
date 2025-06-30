#![allow(clippy::len_without_is_empty)]

use core::ops::*;
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, Noun};
use std::fmt::Debug;
use std::slice::Iter;
pub use nbx_tip5::melt::Melt;

use crate::hand::handle::new_handle_mut_felt;

use super::fext::{finv_, fpow_};
use super::{binv, bpow, FieldError};

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Default, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(transparent)]
pub struct Belt(pub u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Default, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(transparent)]
pub struct Felt(pub [Belt; 3]);

pub trait ElementEx:
    'static
    + Element
    + Copy
    + Mul<Output = Self>
    + Add<Output = Self>
    + Sub<Output = Self>
    + Neg<Output = Self>
    + Div<Output = Self>
    + TryFrom<Noun>
    + Debug
    + Ord
    + Send
    + Sync
{
    fn from_u64(v: u64) -> Self;
    fn epow(&self, p: u64) -> Self;
    fn ordered_root(order: u64) -> Result<Self, FieldError>;
    fn inverse(self) -> Self;
    fn as_noun(self, stack: &mut NockStack) -> Noun;
}

impl ElementEx for Belt {
    fn from_u64(v: u64) -> Self {
        Self(v)
    }

    fn epow(&self, p: u64) -> Self {
        Self(bpow(self.0, p))
    }

    fn ordered_root(order: u64) -> Result<Self, FieldError> {
        Belt(order).ordered_root()
    }

    fn inverse(self) -> Self {
        Self(binv(self.0))
    }

    fn as_noun(self, stack: &mut NockStack) -> Noun {
        Atom::new(stack, self.0).as_noun()
    }
}

impl ElementEx for Melt {
    fn from_u64(v: u64) -> Self {
        Belt::from_u64(v).into()
    }

    fn epow(&self, p: u64) -> Self {
        self.pow(p)
    }

    fn ordered_root(order: u64) -> Result<Self, FieldError> {
        Belt(order).ordered_root().map(Melt::from)
    }

    fn inverse(self) -> Self {
        self.inv()
    }

    fn as_noun(self, stack: &mut NockStack) -> Noun {
        Atom::new(stack, Belt::from(self).0).as_noun()
    }
}

impl ElementEx for Felt {
    fn from_u64(v: u64) -> Self {
        Self::lift(Belt(v))
    }

    fn epow(&self, p: u64) -> Self {
        fpow_(self, p)
    }

    fn ordered_root(order: u64) -> Result<Self, FieldError> {
        Felt::ordered_root(order)
    }

    fn inverse(self) -> Self {
        finv_(&self)
    }

    fn as_noun(self, stack: &mut NockStack) -> Noun {
        let (r, h) = new_handle_mut_felt(stack);
        *h = self;
        r.as_noun()
    }
}

pub trait Element: Clone {
    fn is_zero(&self) -> bool;
    fn zero() -> Self;
    fn len() -> usize;
    fn one() -> Self;
}

impl Element for Felt {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.is_zero()
    }
    #[inline(always)]
    fn zero() -> Self {
        Felt::zero()
    }
    #[inline(always)]
    fn len() -> usize {
        3
    }
    #[inline(always)]
    fn one() -> Self {
        Felt::one()
    }
}

impl Element for Belt {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.is_zero()
    }
    #[inline(always)]
    fn zero() -> Self {
        Belt::zero()
    }
    #[inline(always)]
    fn len() -> usize {
        1
    }
    #[inline(always)]
    fn one() -> Self {
        Belt::one()
    }
}

impl Element for Melt {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.0 == Melt::zero().0
    }
    #[inline(always)]
    fn zero() -> Self {
        Belt::zero().into()
    }
    #[inline(always)]
    fn len() -> usize {
        1
    }
    #[inline(always)]
    fn one() -> Self {
        Belt::one().into()
    }
}

impl Element for u64 {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        *self == 0
    }
    #[inline(always)]
    fn zero() -> Self {
        0
    }
    #[inline(always)]
    fn len() -> usize {
        1
    }
    #[inline(always)]
    fn one() -> Self {
        1
    }
}

pub trait Poly {
    type Element: Element;

    fn data(&self) -> &[Self::Element];

    #[inline(always)]
    fn degree(&self) -> u32 {
        self.data()
            .iter()
            .rposition(|x| !Element::is_zero(x))
            .map_or(0, |i| i as u32)
    }
    #[inline(always)]
    fn leading_coeff(&self) -> &Self::Element {
        &self.data()[self.degree() as usize]
    }
    #[inline(always)]
    fn is_zero(&self) -> bool {
        let len = self.len();
        let data = self.data();
        if len == 0 || (len == 1 && data[0].is_zero()) {
            return true;
        }
        data.iter().all(|x| x.is_zero())
    }
    #[inline(always)]
    fn len(&self) -> usize {
        self.data().len()
    }
    #[inline(always)]
    fn iter(&self) -> Iter<'_, Self::Element> {
        self.data().iter()
    }
}

impl<T> Poly for &[T]
where
    T: Element,
{
    type Element = T;
    #[inline(always)]
    fn data(&self) -> &[T] {
        self
    }
}

impl<T> Poly for Vec<T>
where
    T: Element,
{
    type Element = T;
    #[inline(always)]
    fn data(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Poly for &mut [T]
where
    T: Element,
{
    type Element = T;
    #[inline(always)]
    fn data(&self) -> &[T] {
        self
    }
}

// Wrapper types for Polys to convert from Cell. Only called from top level jet wrapper or in tests.
// Note that form/math functions will always use slice primitives like &[Felt] and &mut [Felt]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct PolyVec<T>(pub Vec<T>);

impl<T> PolyVec<T> {
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PolySlice<'a, T>(pub &'a [T]);

#[repr(transparent)]
pub struct PolySliceMut<'a, T>(pub &'a mut [T]);

pub type BPolyVec = PolyVec<Belt>;
pub type BPolySlice<'a> = PolySlice<'a, Belt>;
pub type BPolySliceMut<'a> = PolySliceMut<'a, Belt>;

pub type MPolyVec = PolyVec<Melt>;
pub type MPolySlice<'a> = PolySlice<'a, Melt>;
pub type MPolySliceMut<'a> = PolySliceMut<'a, Melt>;

pub type FPolyVec = PolyVec<Felt>;
pub type FPolySlice<'a> = PolySlice<'a, Felt>;
pub type FPolySliceMut<'a> = PolySliceMut<'a, Felt>;

impl<T> Poly for PolyVec<T>
where
    T: Element,
{
    type Element = T;
    #[inline(always)]
    fn data(&self) -> &[Self::Element] {
        &self.0
    }
}

impl<T: Element> Poly for PolySlice<'_, T> {
    type Element = T;

    #[inline(always)]
    fn data(&self) -> &[T] {
        self.0
    }
}

impl<T: Element> Poly for PolySliceMut<'_, T> {
    type Element = T;

    #[inline(always)]
    fn data(&self) -> &[T] {
        self.0
    }
}

// For testing BPolyVec funcs
impl From<Vec<u64>> for BPolyVec {
    fn from(b: Vec<u64>) -> Self {
        let belts: Vec<Belt> = b.into_iter().map(|item| item.into()).collect();
        PolyVec(belts)
    }
}

impl From<BPolyVec> for Vec<u64> {
    fn from(b: BPolyVec) -> Self {
        let belts: Vec<u64> = b.0.into_iter().map(|item| item.into()).collect();
        belts
    }
}

impl From<Felt> for BPolyVec {
    fn from(b: Felt) -> Self {
        PolyVec(vec![b.0[0], b.0[1], b.0[2]])
    }
}

impl<'a> From<&'a Felt> for BPolySlice<'a> {
    fn from(f: &'a Felt) -> Self {
        PolySlice(&f.0)
    }
}

impl<'a, T> From<PolySliceMut<'a, T>> for PolySlice<'a, T> {
    fn from(p: PolySliceMut<'a, T>) -> Self {
        Self(p.0)
    }
}

impl<'a, T> From<&'a PolySliceMut<'_, T>> for PolySlice<'a, T> {
    fn from(p: &'a PolySliceMut<'_, T>) -> Self {
        Self(p.0)
    }
}

impl<'a, T> From<&'a PolyVec<T>> for PolySlice<'a, T> {
    fn from(p: &'a PolyVec<T>) -> Self {
        Self(&p.0)
    }
}

impl<'a, T> From<&'a mut PolyVec<T>> for PolySliceMut<'a, T> {
    fn from(p: &'a mut PolyVec<T>) -> Self {
        Self(&mut p.0)
    }
}

impl From<PolyVec<Melt>> for PolyVec<Belt> {
    fn from(mut v: PolyVec<Melt>) -> Self {
        v.0.iter_mut().for_each(|v| *v = Melt(Belt::from(*v).0));
        unsafe { core::mem::transmute(v) }
    }
}

impl From<PolyVec<Belt>> for PolyVec<Melt> {
    fn from(mut v: PolyVec<Belt>) -> Self {
        v.0.iter_mut().for_each(|v| *v = Belt(Melt::from(*v).0));
        unsafe { core::mem::transmute(v) }
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for BPolyVec {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        PolyVec(Vec::<Belt>::arbitrary(g))
    }
}
