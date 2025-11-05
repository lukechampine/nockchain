use crate::mary::{MarySlice, MarySliceMut};

pub unsafe fn mary_transpose_offset_1_2x2(fpolys: MarySlice, res: &mut MarySliceMut) {
    use std::arch::aarch64::*;

    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    let src_ptr = fpolys.dat.as_ptr();
    let dst_ptr = res.dat.as_mut_ptr();

    // Process 2x2 blocks with NEON (2 u64s per vector)
    const BLOCK: usize = 2;

    let cols_blocked = (num_cols / BLOCK) * BLOCK;
    let rows_blocked = (num_rows / BLOCK) * BLOCK;

    // Main NEON loop - 2x2 blocks
    for i in (0..cols_blocked).step_by(BLOCK) {
        for j in (0..rows_blocked).step_by(BLOCK) {
            // Load 2x2 block from source
            // Row 0: [src[j*cols + i], src[j*cols + i+1]]
            // Row 1: [src[(j+1)*cols + i], src[(j+1)*cols + i+1]]

            let row0 = vld1q_u64(src_ptr.add(j * num_cols + i));
            let row1 = vld1q_u64(src_ptr.add((j + 1) * num_cols + i));

            // Transpose 2x2:
            // Before: row0 = [a, b], row1 = [c, d]
            // After:  col0 = [a, c], col1 = [b, d]

            // Create interleaved vectors
            let col0 = vcombine_u64(vget_low_u64(row0), vget_low_u64(row1)); // [a, c]
            let col1 = vcombine_u64(vget_high_u64(row0), vget_high_u64(row1)); // [b, d]

            // Store transposed 2x2 block
            vst1q_u64(dst_ptr.add(i * num_rows + j), col0);
            vst1q_u64(dst_ptr.add((i + 1) * num_rows + j), col1);
        }

        // Handle remaining rows in this column block
        for j in rows_blocked..num_rows {
            *dst_ptr.add(i * num_rows + j) = *src_ptr.add(j * num_cols + i);
            *dst_ptr.add((i + 1) * num_rows + j) = *src_ptr.add(j * num_cols + i + 1);
        }
    }

    // Handle remaining columns
    for i in cols_blocked..num_cols {
        for j in 0..num_rows {
            *dst_ptr.add(i * num_rows + j) = *src_ptr.add(j * num_cols + i);
        }
    }
}

pub unsafe fn mary_transpose_offset_1_blocked_2x2<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_neon_2x2(
                fpolys.dat, res.dat, num_cols, num_rows, i_tile, i_end, j_tile, j_end,
            );
        }
    }
}

pub unsafe fn mary_transpose_offset_1_blocked_4x2<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_neon_4x2(
                fpolys.dat, res.dat, num_cols, num_rows, i_tile, i_end, j_tile, j_end,
            );
        }
    }
}

pub unsafe fn mary_transpose_offset_1_blocked_4x4<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_neon_4x4(
                fpolys.dat, res.dat, num_cols, num_rows, i_tile, i_end, j_tile, j_end,
            );
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn transpose_tile_neon_2x2(
    src: &[u64],
    dst: &mut [u64],
    num_cols: usize,
    num_rows: usize,
    i_start: usize,
    i_end: usize,
    j_start: usize,
    j_end: usize,
) {
    use std::arch::aarch64::*;

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();

    let tile_cols = i_end - i_start;
    let tile_rows = j_end - j_start;

    // Process 2x2 NEON blocks
    let mut i = 0;
    while i + 2 <= tile_cols {
        let global_i = i_start + i;

        let mut j = 0;
        while j + 2 <= tile_rows {
            let global_j = j_start + j;

            // Load 2x2 block
            let row0 = vld1q_u64(src_ptr.add(global_j * num_cols + global_i));
            let row1 = vld1q_u64(src_ptr.add((global_j + 1) * num_cols + global_i));

            // Transpose: [a,b] [c,d] -> [a,c] [b,d]
            let col0 = vcombine_u64(vget_low_u64(row0), vget_low_u64(row1));
            let col1 = vcombine_u64(vget_high_u64(row0), vget_high_u64(row1));

            // Store
            vst1q_u64(dst_ptr.add(global_i * num_rows + global_j), col0);
            vst1q_u64(dst_ptr.add((global_i + 1) * num_rows + global_j), col1);

            j += 2;
        }

        // Handle remaining rows
        while j < tile_rows {
            let global_j = j_start + j;
            *dst_ptr.add(global_i * num_rows + global_j) =
                *src_ptr.add(global_j * num_cols + global_i);
            *dst_ptr.add((global_i + 1) * num_rows + global_j) =
                *src_ptr.add(global_j * num_cols + global_i + 1);
            j += 1;
        }

        i += 2;
    }

    // Handle remaining columns
    while i < tile_cols {
        let global_i = i_start + i;
        for j in 0..tile_rows {
            let global_j = j_start + j;
            *dst_ptr.add(global_i * num_rows + global_j) =
                *src_ptr.add(global_j * num_cols + global_i);
        }
        i += 1;
    }
}

unsafe fn transpose_tile_neon_4x2(
    src: &[u64],
    dst: &mut [u64],
    num_cols: usize,
    num_rows: usize,
    i_start: usize,
    i_end: usize,
    j_start: usize,
    j_end: usize,
) {
    use std::arch::aarch64::*;

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();

    let tile_cols = i_end - i_start;
    let tile_rows = j_end - j_start;

    let mut i = 0;
    while i + 4 <= tile_cols {
        let global_i = i_start + i;

        let mut j = 0;
        while j + 2 <= tile_rows {
            let global_j = j_start + j;

            // Load 4x2 block
            let row0_0 = vld1q_u64(src_ptr.add(global_j * num_cols + global_i));
            let row0_1 = vld1q_u64(src_ptr.add(global_j * num_cols + global_i + 2));
            let row1_0 = vld1q_u64(src_ptr.add((global_j + 1) * num_cols + global_i));
            let row1_1 = vld1q_u64(src_ptr.add((global_j + 1) * num_cols + global_i + 2));

            // Transpose
            let col0 = vcombine_u64(vget_low_u64(row0_0), vget_low_u64(row1_0));
            let col1 = vcombine_u64(vget_high_u64(row0_0), vget_high_u64(row1_0));
            let col2 = vcombine_u64(vget_low_u64(row0_1), vget_low_u64(row1_1));
            let col3 = vcombine_u64(vget_high_u64(row0_1), vget_high_u64(row1_1));

            // Store
            vst1q_u64(dst_ptr.add(global_i * num_rows + global_j), col0);
            vst1q_u64(dst_ptr.add((global_i + 1) * num_rows + global_j), col1);
            vst1q_u64(dst_ptr.add((global_i + 2) * num_rows + global_j), col2);
            vst1q_u64(dst_ptr.add((global_i + 3) * num_rows + global_j), col3);

            j += 2;
        }

        // Remaining rows
        while j < tile_rows {
            let global_j = j_start + j;
            for k in 0..4 {
                *dst_ptr.add((global_i + k) * num_rows + global_j) =
                    *src_ptr.add(global_j * num_cols + global_i + k);
            }
            j += 1;
        }

        i += 4;
    }

    // Remaining columns
    while i < tile_cols {
        let global_i = i_start + i;
        for j in 0..tile_rows {
            let global_j = j_start + j;
            *dst_ptr.add(global_i * num_rows + global_j) =
                *src_ptr.add(global_j * num_cols + global_i);
        }
        i += 1;
    }
}

unsafe fn transpose_tile_neon_4x4(
    src: &[u64],
    dst: &mut [u64],
    num_cols: usize,
    num_rows: usize,
    i_start: usize,
    i_end: usize,
    j_start: usize,
    j_end: usize,
) {
    use std::arch::aarch64::*;

    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();

    let tile_cols = i_end - i_start;
    let tile_rows = j_end - j_start;

    let mut i = 0;
    while i + 4 <= tile_cols {
        let global_i = i_start + i;

        let mut j = 0;
        while j + 4 <= tile_rows {
            let global_j = j_start + j;

            // Load 4x4 block (4 rows, 4 cols each = 8 u64 loads)
            let r0_0 = vld1q_u64(src_ptr.add(global_j * num_cols + global_i));
            let r0_1 = vld1q_u64(src_ptr.add(global_j * num_cols + global_i + 2));
            let r1_0 = vld1q_u64(src_ptr.add((global_j + 1) * num_cols + global_i));
            let r1_1 = vld1q_u64(src_ptr.add((global_j + 1) * num_cols + global_i + 2));
            let r2_0 = vld1q_u64(src_ptr.add((global_j + 2) * num_cols + global_i));
            let r2_1 = vld1q_u64(src_ptr.add((global_j + 2) * num_cols + global_i + 2));
            let r3_0 = vld1q_u64(src_ptr.add((global_j + 3) * num_cols + global_i));
            let r3_1 = vld1q_u64(src_ptr.add((global_j + 3) * num_cols + global_i + 2));

            // Transpose 4x4 in two 2x2 blocks
            // First 2x2 (cols 0-1)
            let c0_01 = vcombine_u64(vget_low_u64(r0_0), vget_low_u64(r1_0));
            let c1_01 = vcombine_u64(vget_high_u64(r0_0), vget_high_u64(r1_0));
            let c0_23 = vcombine_u64(vget_low_u64(r2_0), vget_low_u64(r3_0));
            let c1_23 = vcombine_u64(vget_high_u64(r2_0), vget_high_u64(r3_0));

            // Second 2x2 (cols 2-3)
            let c2_01 = vcombine_u64(vget_low_u64(r0_1), vget_low_u64(r1_1));
            let c3_01 = vcombine_u64(vget_high_u64(r0_1), vget_high_u64(r1_1));
            let c2_23 = vcombine_u64(vget_low_u64(r2_1), vget_low_u64(r3_1));
            let c3_23 = vcombine_u64(vget_high_u64(r2_1), vget_high_u64(r3_1));

            // Interleave to get final columns
            let col0 = vcombine_u64(vget_low_u64(c0_01), vget_low_u64(c0_23));
            let col1 = vcombine_u64(vget_low_u64(c1_01), vget_low_u64(c1_23));
            let col2 = vcombine_u64(vget_low_u64(c2_01), vget_low_u64(c2_23));
            let col3 = vcombine_u64(vget_low_u64(c3_01), vget_low_u64(c3_23));

            // Store - need to store in 2 chunks since we have 4 elements per column
            vst1q_u64(dst_ptr.add(global_i * num_rows + global_j), col0);
            vst1q_u64(
                dst_ptr.add(global_i * num_rows + global_j + 2),
                vcombine_u64(vget_high_u64(c0_01), vget_high_u64(c0_23)),
            );

            vst1q_u64(dst_ptr.add((global_i + 1) * num_rows + global_j), col1);
            vst1q_u64(
                dst_ptr.add((global_i + 1) * num_rows + global_j + 2),
                vcombine_u64(vget_high_u64(c1_01), vget_high_u64(c1_23)),
            );

            vst1q_u64(dst_ptr.add((global_i + 2) * num_rows + global_j), col2);
            vst1q_u64(
                dst_ptr.add((global_i + 2) * num_rows + global_j + 2),
                vcombine_u64(vget_high_u64(c2_01), vget_high_u64(c2_23)),
            );

            vst1q_u64(dst_ptr.add((global_i + 3) * num_rows + global_j), col3);
            vst1q_u64(
                dst_ptr.add((global_i + 3) * num_rows + global_j + 2),
                vcombine_u64(vget_high_u64(c3_01), vget_high_u64(c3_23)),
            );

            j += 4;
        }

        // Remaining rows
        while j < tile_rows {
            let global_j = j_start + j;
            for k in 0..4 {
                *dst_ptr.add((global_i + k) * num_rows + global_j) =
                    *src_ptr.add(global_j * num_cols + global_i + k);
            }
            j += 1;
        }

        i += 4;
    }

    // Remaining columns
    while i < tile_cols {
        let global_i = i_start + i;
        for j in 0..tile_rows {
            let global_j = j_start + j;
            *dst_ptr.add(global_i * num_rows + global_j) =
                *src_ptr.add(global_j * num_cols + global_i);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data(rows: usize, cols: usize) -> Vec<u64> {
        (0..rows * cols).map(|i| i as u64).collect()
    }

    #[test]
    fn test_neon_mary_transpose_offset_1_2x2() {
        let rows = 195;
        let cols = 65536;

        let src = create_test_data(rows, cols);
        let mut dst = vec![0u64; rows * cols];

        let fpolys = MarySlice {
            dat: &src,
            len: rows as u32,
            step: cols as u32,
        };

        let mut res = MarySliceMut {
            dat: &mut dst,
            len: cols as u32,
            step: rows as u32,
        };
    }
}
