use crate::poly_ext::ElementEx;

#[inline(always)]
pub fn dense<T: ElementEx>(x: &mut [T], twiddles: &[impl AsRef<[T]>], log_2_of_n: u32) {
    for stage_idx in 0..log_2_of_n {
        let twiddles = twiddles[stage_idx as usize].as_ref();
        assert!(twiddles.len().is_power_of_two());
        for uv in x.chunks_exact_mut(2 * twiddles.len()) {
            let (u, v) = uv.split_at_mut(twiddles.len());
            for (w, (u_mut, v_mut)) in twiddles.iter().copied().zip(u.iter_mut().zip(v.iter_mut()))
            {
                let u = *u_mut;
                let v = *v_mut * w;
                *u_mut = u + v;
                *v_mut = u - v;
            }
        }
    }
}

#[inline(always)]
pub fn sparse<T: ElementEx>(
    x: &mut [T],
    twiddles: &[impl AsRef<[T]>],
    log_2_of_n: u32,
    last_non_zero_index: usize,
    bit_reverse: &[u32],
) {
    // Twiddle operations spread out non-zero values throughout the vector. However, during the first
    //  iteration, this can be very sparse and a lot of operations can be skipped.
    let twiddles_stage0 = twiddles[0].as_ref();
    debug_assert_eq!(twiddles_stage0.len(), 1); // Should be [1]

    for i in 0..=last_non_zero_index {
        let i = bit_reverse[i] as usize;
        let j = i + 1;

        // Note that w = 1 for stage 0
        let u_val = x[i];
        let v_val = x[j];
        x[i] = u_val + v_val;
        x[j] = u_val - v_val;
    }

    // Process the remaining stages normally
    for stage_idx in 1..log_2_of_n {
        let twiddles_stage = twiddles[stage_idx as usize].as_ref();
        for uv in x.chunks_exact_mut(2 * twiddles_stage.len()) {
            let (u, v) = uv.split_at_mut(twiddles_stage.len());
            for (w, (u_mut, v_mut)) in twiddles_stage
                .iter()
                .copied()
                .zip(u.iter_mut().zip(v.iter_mut()))
            {
                let u_val = *u_mut;
                let v_val = *v_mut * w;
                *u_mut = u_val + v_val;
                *v_mut = u_val - v_val;
            }
        }
    }
}
