use std::simd::prelude::*;

use nbx_tip5::base::{montiply_simd_x2, montiply_simd_x8};
use nbx_tip5::melt::Melt;

#[inline]
pub fn p_hadamard_inplace_4096_melt_x8(a: &mut [Melt], b: &[Melt]) {
    assert_eq!(a.len(), 4096);

    unsafe {
        let a_ptr = a.as_mut_ptr() as *mut u64x8;
        let b_ptr = b.as_ptr() as *const u64x8;

        for i in 0..512 {
            let a_vals = a_ptr.add(i).read_unaligned();
            let b_vals = b_ptr.add(i).read_unaligned();
            a_ptr
                .add(i)
                .write_unaligned(montiply_simd_x8(a_vals, b_vals));
        }
    }
}

#[inline]
pub fn p_hadamard_inplace_4096_melt_x2(a: &mut [Melt], b: &[Melt]) {
    assert_eq!(a.len(), 4096);

    unsafe {
        let a_ptr = a.as_mut_ptr() as *mut u64x2;
        let b_ptr = b.as_ptr() as *const u64x2;

        for i in 0..2048 {
            // 4096 / 2
            let a_vals = a_ptr.add(i).read_unaligned();
            let b_vals = b_ptr.add(i).read_unaligned();
            a_ptr
                .add(i)
                .write_unaligned(montiply_simd_x2(a_vals, b_vals));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hadamard::scalar::p_hadamard_inplace;

    #[test]
    fn test_hadamard_variants_match() {
        let a_orig: Vec<Melt> = (0..4096).map(|i| Melt((i * 7 + 13) as u64)).collect();
        let b: Vec<Melt> = (0..4096).map(|i| Melt((i * 3 + 5) as u64)).collect();

        // Scalar baseline
        let mut a_scalar = a_orig.clone();
        p_hadamard_inplace(&mut a_scalar, &b);

        // x2 SIMD
        let mut a_x2 = a_orig.clone();
        p_hadamard_inplace_4096_melt_x2(&mut a_x2, &b);

        // x8 SIMD
        let mut a_x8 = a_orig.clone();
        p_hadamard_inplace_4096_melt_x8(&mut a_x8, &b);

        assert_eq!(a_scalar, a_x2, "x2 SIMD doesn't match scalar");
        assert_eq!(a_scalar, a_x8, "x8 SIMD doesn't match scalar");
    }
}
