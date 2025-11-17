use std::simd::prelude::*;

use nbx_tip5::base::{badd_simd, bmul_simd_x8, bsub_simd};

use crate::belt::Belt;

#[inline(always)]
pub fn sparse_x8<W: AsRef<[Belt]>>(
    x: &mut [Belt],
    twiddles: &[W],
    log_2_of_n: u32,
    last_non_zero_index: usize,
    bit_reverse: &[u32],
) {
    // First stage - handle sparse data with bit-reversed indices (scalar version)
    let twiddles_stage0 = twiddles[0].as_ref();
    debug_assert_eq!(twiddles_stage0.len(), 1);

    for i in 0..=last_non_zero_index {
        let i = bit_reverse[i] as usize;
        let j = i + 1;

        let u_val = x[i];
        let v_val = x[j];
        x[i] = u_val + v_val;
        x[j] = u_val - v_val;
    }

    // Process remaining stages with SIMD
    for stage_idx in 1..log_2_of_n {
        let twiddles_stage = twiddles[stage_idx as usize].as_ref();
        let chunk_size = 2 * twiddles_stage.len();

        for uv in x.chunks_exact_mut(chunk_size) {
            let (u, v) = uv.split_at_mut(twiddles_stage.len());

            // Cast Belt slices to u64 slices for direct SIMD access
            // Safety: Belt is repr(transparent) over u64
            let u_raw =
                unsafe { std::slice::from_raw_parts_mut(u.as_mut_ptr() as *mut u64, u.len()) };
            let v_raw =
                unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u64, v.len()) };
            let w_raw = unsafe {
                std::slice::from_raw_parts(
                    twiddles_stage.as_ptr() as *const u64,
                    twiddles_stage.len(),
                )
            };

            // Process 8 elements at a time with SIMD
            let simd_chunks = twiddles_stage.len() / 8;
            let remainder = twiddles_stage.len() % 8;

            // SIMD processing for groups of 8
            for i in 0..simd_chunks {
                let base_idx = i * 8;

                let u_vals = u64x8::from_slice(&u_raw[base_idx..]);
                let v_vals = u64x8::from_slice(&v_raw[base_idx..]);
                let w_vals = u64x8::from_slice(&w_raw[base_idx..]);

                // Perform SIMD butterfly operation
                let v_times_w = bmul_simd_x8(v_vals, w_vals);
                let u_new = badd_simd(u_vals, v_times_w);
                let v_new = bsub_simd(u_vals, v_times_w);

                u_new.copy_to_slice(&mut u_raw[base_idx..]);
                v_new.copy_to_slice(&mut v_raw[base_idx..]);
            }

            // Handle remaining elements with scalar code
            let base_idx = simd_chunks * 8;
            for i in 0..remainder {
                let w = twiddles_stage[base_idx + i];
                let u_val = u[base_idx + i];
                let v_val = v[base_idx + i] * w;
                u[base_idx + i] = u_val + v_val;
                v[base_idx + i] = u_val - v_val;
            }
        }
    }
}

#[inline(always)]
pub fn dense_x8(x: &mut [Belt], twiddles: &[impl AsRef<[Belt]>], log_2_of_n: u32) {
    for stage_idx in 0..log_2_of_n {
        let twiddles_stage = twiddles[stage_idx as usize].as_ref();
        assert!(twiddles_stage.len().is_power_of_two());

        for uv in x.chunks_exact_mut(2 * twiddles_stage.len()) {
            let (u, v) = uv.split_at_mut(twiddles_stage.len());

            // Cast to raw u64 slices for direct SIMD access
            let u_raw =
                unsafe { std::slice::from_raw_parts_mut(u.as_mut_ptr() as *mut u64, u.len()) };
            let v_raw =
                unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u64, v.len()) };
            let w_raw = unsafe {
                std::slice::from_raw_parts(
                    twiddles_stage.as_ptr() as *const u64,
                    twiddles_stage.len(),
                )
            };

            // Process 8 elements at a time with SIMD
            let simd_chunks = twiddles_stage.len() / 8;
            let remainder = twiddles_stage.len() % 8;

            for i in 0..simd_chunks {
                let base_idx = i * 8;

                // Direct SIMD loads
                let u_vals = u64x8::from_slice(&u_raw[base_idx..]);
                let v_vals = u64x8::from_slice(&v_raw[base_idx..]);
                let w_vals = u64x8::from_slice(&w_raw[base_idx..]);

                // Butterfly operation
                let v_times_w = bmul_simd_x8(v_vals, w_vals);
                let u_new = badd_simd(u_vals, v_times_w);
                let v_new = bsub_simd(u_vals, v_times_w);

                // Direct SIMD stores
                u_new.copy_to_slice(&mut u_raw[base_idx..]);
                v_new.copy_to_slice(&mut v_raw[base_idx..]);
            }

            // Scalar remainder
            let base_idx = simd_chunks * 8;
            for i in 0..remainder {
                let w = twiddles_stage[base_idx + i];
                let u_val = u[base_idx + i];
                let v_val = v[base_idx + i] * w;
                u[base_idx + i] = u_val + v_val;
                v[base_idx + i] = u_val - v_val;
            }
        }
    }
}
