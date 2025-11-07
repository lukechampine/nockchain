use crate::poly_ext::ElementEx;

#[inline]
pub fn p_hadamard_inplace<T: ElementEx + 'static, O: Copy + Into<T> + 'static>(
    a: &mut [T],
    b: &[O],
) {
    for (a, b) in a.iter_mut().zip(b) {
        *a *= (*b).into();
    }
}

#[inline]
pub fn p_hadamard_inplace_chunked<T: ElementEx + 'static, O: Copy + Into<T> + 'static>(
    a: &mut [T],
    b: &[O],
) {
    const CHUNK_SIZE: usize = 16;
    let (a_chunks, a_chunks_rem) = a.as_chunks_mut::<CHUNK_SIZE>();
    let (b_chunks, b_chunks_rem) = b.as_chunks::<CHUNK_SIZE>();

    for (a, b) in a_chunks.iter_mut().zip(b_chunks) {
        for (a, b) in a.iter_mut().zip(b) {
            *a *= (*b).into();
        }
    }

    for (a, b) in a_chunks_rem.iter_mut().zip(b_chunks_rem) {
        *a *= (*b).into();
    }
}

#[inline]
#[rustfmt::skip]
pub fn p_hadamard_inplace_chunked_prefetched<T: ElementEx + 'static, O: Copy + Into<T> + 'static>(
    a: &mut [T],
    b: &[O],
) {
    const CHUNK_SIZE: usize = 16;
    const PREFETCH_CHUNKS: usize = 4;
    let num_chunks = a.len() / CHUNK_SIZE;
    let (a_chunks, a_chunks_rem) = a.as_chunks_mut::<CHUNK_SIZE>();
    let (b_chunks, b_chunks_rem) = b.as_chunks::<CHUNK_SIZE>();

    for (i, (a, b)) in a_chunks.iter_mut().zip(b_chunks).enumerate() {
        if i + PREFETCH_CHUNKS < num_chunks {
            unsafe {
                #[cfg(target_arch = "aarch64")]
                std::arch::asm!(
                "prfm pldl1keep, [{0}]",
                "prfm pldl1strm, [{1}]",
                in(reg) a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS),
                in(reg) b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS),
                options(nostack, readonly)
                );

                #[cfg(target_arch = "x86_64")]
                {
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_ET0 }>(a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_ET0 }>(a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS + CHUNK_SIZE / 2) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS + CHUNK_SIZE / 2) as *const _);
                }
            }
        }

        for (a, b) in a.iter_mut().zip(b) {
            *a *= (*b).into();
        }
    }

    for (a, b) in a_chunks_rem
        .iter_mut()
        .zip(b_chunks_rem)
    {
        *a *= (*b).into();
    }
}

#[cfg(test)]
mod tests {
    use nbx_tip5::melt::Melt;

    use super::*;

    #[test]
    fn test_hadamard_variants_match() {
        let a_orig: Vec<Melt> = (0..4096).map(|i| Melt((i * 7 + 13) as u64)).collect();
        let b: Vec<Melt> = (0..4096).map(|i| Melt((i * 3 + 5) as u64)).collect();

        // Scalar baseline
        let mut a_scalar = a_orig.clone();
        p_hadamard_inplace(&mut a_scalar, &b);

        // Scalar chunked
        let mut a_chunked = a_orig.clone();
        p_hadamard_inplace_chunked(&mut a_chunked, &b);

        // Scalar chunked prefetched
        let mut a_chunked_prefetched = a_orig.clone();
        p_hadamard_inplace_chunked_prefetched(&mut a_chunked_prefetched, &b);

        assert_eq!(a_scalar, a_chunked);
        assert_eq!(a_scalar, a_chunked_prefetched);
    }
}
