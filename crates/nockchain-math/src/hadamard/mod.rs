use std::any::TypeId;

use nbx_tip5::melt::Melt;

use crate::poly_ext::ElementEx;

pub mod scalar;
pub mod simd;

#[inline]
pub fn p_hadamard_inplace<T: ElementEx + 'static, O: Copy + Into<T> + 'static>(
    a: &mut [T],
    b: &[O],
) {
    assert_eq!(
        a.len(),
        b.len(),
        "Unequal lengths: {}, {}",
        a.len(),
        b.len()
    );

    #[cfg(target_feature = "avx512f")]
    if TypeId::of::<T>() == TypeId::of::<Melt>()
        && TypeId::of::<O>() == TypeId::of::<Melt>()
        && a.len() == 4096
    {
        unsafe {
            let a_melt = &mut *(a as *mut [T] as *mut [Melt]);
            let b_melt = &*(b as *const [O] as *const [Melt]);
            simd::p_hadamard_inplace_4096_melt_x8(a_melt, b_melt);
            return;
        }
    }

    scalar::p_hadamard_inplace_chunked_prefetched(a, b)
}
