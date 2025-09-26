pub use nbx_tip5::melt::Melt;

use crate::belt::Belt;

impl From<Melt> for Belt {
    #[inline(always)]
    fn from(value: Melt) -> Self {
        Belt(nbx_tip5::base::mont_reduction(value.0 as _))
    }
}

impl From<Belt> for Melt {
    #[inline(always)]
    fn from(value: Belt) -> Self {
        Melt::from_u64(value.0)
    }
}
