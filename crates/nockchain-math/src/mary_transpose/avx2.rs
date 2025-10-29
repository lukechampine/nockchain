use crate::mary::{MarySlice, MarySliceMut};

pub unsafe fn mary_transpose_offset_1_4x4(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    use std::arch::x86_64::*;

    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    let src_ptr = fpolys.dat.as_ptr();
    let dst_ptr = res.dat.as_mut_ptr();

    const BLOCK: usize = 4;
    let cols_blocked = (num_cols / BLOCK) * BLOCK;
    let rows_blocked = (num_rows / BLOCK) * BLOCK;

    // Main AVX2 loop - 4x4 blocks
    for i in (0..cols_blocked).step_by(BLOCK) {
        for j in (0..rows_blocked).step_by(BLOCK) {
            // Load 4x4 block
            let row0 = _mm256_loadu_si256(src_ptr.add(j * num_cols + i) as *const __m256i);
            let row1 = _mm256_loadu_si256(src_ptr.add((j + 1) * num_cols + i) as *const __m256i);
            let row2 = _mm256_loadu_si256(src_ptr.add((j + 2) * num_cols + i) as *const __m256i);
            let row3 = _mm256_loadu_si256(src_ptr.add((j + 3) * num_cols + i) as *const __m256i);

            // Transpose 4x4
            let tmp0 = _mm256_unpacklo_epi64(row0, row1); // [a0, b0, a2, b2]
            let tmp1 = _mm256_unpackhi_epi64(row0, row1); // [a1, b1, a3, b3]
            let tmp2 = _mm256_unpacklo_epi64(row2, row3); // [c0, d0, c2, d2]
            let tmp3 = _mm256_unpackhi_epi64(row2, row3); // [c1, d1, c3, d3]

            let col0 = _mm256_permute2x128_si256(tmp0, tmp2, 0x20); // [a0, b0, c0, d0]
            let col1 = _mm256_permute2x128_si256(tmp1, tmp3, 0x20); // [a1, b1, c1, d1]
            let col2 = _mm256_permute2x128_si256(tmp0, tmp2, 0x31); // [a2, b2, c2, d2]
            let col3 = _mm256_permute2x128_si256(tmp1, tmp3, 0x31); // [a3, b3, c3, d3]

            // Store transposed block
            _mm256_storeu_si256(dst_ptr.add(i * num_rows + j) as *mut __m256i, col0);
            _mm256_storeu_si256(dst_ptr.add((i + 1) * num_rows + j) as *mut __m256i, col1);
            _mm256_storeu_si256(dst_ptr.add((i + 2) * num_rows + j) as *mut __m256i, col2);
            _mm256_storeu_si256(dst_ptr.add((i + 3) * num_rows + j) as *mut __m256i, col3);
        }

        // Handle remaining rows
        for j in rows_blocked..num_rows {
            for k in 0..BLOCK {
                *dst_ptr.add((i + k) * num_rows + j) = *src_ptr.add(j * num_cols + i + k);
            }
        }
    }

    // Handle remaining columns
    for i in cols_blocked..num_cols {
        for j in 0..num_rows {
            *dst_ptr.add(i * num_rows + j) = *src_ptr.add(j * num_cols + i);
        }
    }
}

pub unsafe fn mary_transpose_offset_1_blocked_4x4_avx2<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_4x4(
                fpolys.dat, res.dat,
                num_cols, num_rows,
                i_tile, i_end, j_tile, j_end
            );
        }
    }
}

unsafe fn transpose_tile_4x4(
    src: &[u64],
    dst: &mut [u64],
    num_cols: usize,
    num_rows: usize,
    i_start: usize,
    i_end: usize,
    j_start: usize,
    j_end: usize,
) {
    use std::arch::x86_64::*;

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

            // Load 4x4 block
            let row0 = _mm256_loadu_si256(src_ptr.add(global_j * num_cols + global_i) as *const __m256i);
            let row1 = _mm256_loadu_si256(src_ptr.add((global_j + 1) * num_cols + global_i) as *const __m256i);
            let row2 = _mm256_loadu_si256(src_ptr.add((global_j + 2) * num_cols + global_i) as *const __m256i);
            let row3 = _mm256_loadu_si256(src_ptr.add((global_j + 3) * num_cols + global_i) as *const __m256i);

            // Transpose
            let tmp0 = _mm256_unpacklo_epi64(row0, row1);
            let tmp1 = _mm256_unpackhi_epi64(row0, row1);
            let tmp2 = _mm256_unpacklo_epi64(row2, row3);
            let tmp3 = _mm256_unpackhi_epi64(row2, row3);

            let col0 = _mm256_permute2x128_si256(tmp0, tmp2, 0x20);
            let col1 = _mm256_permute2x128_si256(tmp1, tmp3, 0x20);
            let col2 = _mm256_permute2x128_si256(tmp0, tmp2, 0x31);
            let col3 = _mm256_permute2x128_si256(tmp1, tmp3, 0x31);

            // Store
            _mm256_storeu_si256(dst_ptr.add(global_i * num_rows + global_j) as *mut __m256i, col0);
            _mm256_storeu_si256(dst_ptr.add((global_i + 1) * num_rows + global_j) as *mut __m256i, col1);
            _mm256_storeu_si256(dst_ptr.add((global_i + 2) * num_rows + global_j) as *mut __m256i, col2);
            _mm256_storeu_si256(dst_ptr.add((global_i + 3) * num_rows + global_j) as *mut __m256i, col3);

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

pub unsafe fn mary_transpose_offset_1_4x4_avx2_nt(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    use std::arch::x86_64::*;

    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    let src_ptr = fpolys.dat.as_ptr();
    let dst_ptr = res.dat.as_mut_ptr();

    const BLOCK: usize = 4;
    let cols_blocked = (num_cols / BLOCK) * BLOCK;
    let rows_blocked = (num_rows / BLOCK) * BLOCK;

    for i in (0..cols_blocked).step_by(BLOCK) {
        for j in (0..rows_blocked).step_by(BLOCK) {
            let row0 = _mm256_loadu_si256(src_ptr.add(j * num_cols + i) as *const __m256i);
            let row1 = _mm256_loadu_si256(src_ptr.add((j + 1) * num_cols + i) as *const __m256i);
            let row2 = _mm256_loadu_si256(src_ptr.add((j + 2) * num_cols + i) as *const __m256i);
            let row3 = _mm256_loadu_si256(src_ptr.add((j + 3) * num_cols + i) as *const __m256i);

            let tmp0 = _mm256_unpacklo_epi64(row0, row1);
            let tmp1 = _mm256_unpackhi_epi64(row0, row1);
            let tmp2 = _mm256_unpacklo_epi64(row2, row3);
            let tmp3 = _mm256_unpackhi_epi64(row2, row3);

            let col0 = _mm256_permute2x128_si256(tmp0, tmp2, 0x20);
            let col1 = _mm256_permute2x128_si256(tmp1, tmp3, 0x20);
            let col2 = _mm256_permute2x128_si256(tmp0, tmp2, 0x31);
            let col3 = _mm256_permute2x128_si256(tmp1, tmp3, 0x31);

            _mm256_stream_si256(dst_ptr.add(i * num_rows + j) as *mut __m256i, col0);
            _mm256_stream_si256(dst_ptr.add((i + 1) * num_rows + j) as *mut __m256i, col1);
            _mm256_stream_si256(dst_ptr.add((i + 2) * num_rows + j) as *mut __m256i, col2);
            _mm256_stream_si256(dst_ptr.add((i + 3) * num_rows + j) as *mut __m256i, col3);
        }

        _mm_sfence();

        for j in rows_blocked..num_rows {
            for k in 0..BLOCK {
                *dst_ptr.add((i + k) * num_rows + j) = *src_ptr.add(j * num_cols + i + k);
            }
        }
    }

    for i in cols_blocked..num_cols {
        for j in 0..num_rows {
            *dst_ptr.add(i * num_rows + j) = *src_ptr.add(j * num_cols + i);
        }
    }
}