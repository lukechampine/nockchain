use crate::poly_ext::ElementEx;

pub mod scalar;
pub mod simd;

#[inline(always)]
pub fn dense<T: ElementEx>(x: &mut [T], twiddles: &[impl AsRef<[T]>], log_2_of_n: u32) {
    #[cfg(target_feature = "avx512f")]
    {
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<crate::belt::Belt>() {
            unsafe {
                let x_belt: &mut [crate::belt::Belt] = std::mem::transmute(x);
                let twiddles_belt: Vec<&[crate::belt::Belt]> = twiddles
                    .iter()
                    .map(|t| std::mem::transmute(t.as_ref()))
                    .collect();
                return simd::dense_x8(x_belt, &twiddles_belt, log_2_of_n);
            }
        }
    }

    scalar::dense(x, twiddles, log_2_of_n);
}

#[inline(always)]
pub fn sparse<T: ElementEx>(
    x: &mut [T],
    twiddles: &[impl AsRef<[T]>],
    log_2_of_n: u32,
    last_non_zero_index: usize,
    bit_reverse: &[u32],
) {
    #[cfg(target_feature = "avx512f")]
    {
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<crate::belt::Belt>() {
            unsafe {
                let x_belt: &mut [crate::belt::Belt] = std::mem::transmute(x);
                let twiddles_belt: Vec<&[crate::belt::Belt]> = twiddles
                    .iter()
                    .map(|t| std::mem::transmute(t.as_ref()))
                    .collect();
                return simd::sparse_x8(
                    x_belt, &twiddles_belt, log_2_of_n, last_non_zero_index, bit_reverse,
                );
            }
        }
    }

    scalar::sparse(x, twiddles, log_2_of_n, last_non_zero_index, bit_reverse);
}
