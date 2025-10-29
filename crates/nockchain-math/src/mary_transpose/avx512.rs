use crate::mary::{MarySlice, MarySliceMut};

pub unsafe fn mary_transpose_offset_1_8x8(
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

    const BLOCK: usize = 8;
    let cols_blocked = (num_cols / BLOCK) * BLOCK;
    let rows_blocked = (num_rows / BLOCK) * BLOCK;

    // Main AVX512 loop - 8x8 blocks
    for i in (0..cols_blocked).step_by(BLOCK) {
        for j in (0..rows_blocked).step_by(BLOCK) {
            // Load 8x8 block
            let row0 = _mm512_loadu_si512(src_ptr.add(j * num_cols + i) as *const __m512i);
            let row1 = _mm512_loadu_si512(src_ptr.add((j + 1) * num_cols + i) as *const __m512i);
            let row2 = _mm512_loadu_si512(src_ptr.add((j + 2) * num_cols + i) as *const __m512i);
            let row3 = _mm512_loadu_si512(src_ptr.add((j + 3) * num_cols + i) as *const __m512i);
            let row4 = _mm512_loadu_si512(src_ptr.add((j + 4) * num_cols + i) as *const __m512i);
            let row5 = _mm512_loadu_si512(src_ptr.add((j + 5) * num_cols + i) as *const __m512i);
            let row6 = _mm512_loadu_si512(src_ptr.add((j + 6) * num_cols + i) as *const __m512i);
            let row7 = _mm512_loadu_si512(src_ptr.add((j + 7) * num_cols + i) as *const __m512i);

            // Transpose 8x8 using AVX512 permute operations
            let tmp0 = _mm512_unpacklo_epi64(row0, row1);
            let tmp1 = _mm512_unpackhi_epi64(row0, row1);
            let tmp2 = _mm512_unpacklo_epi64(row2, row3);
            let tmp3 = _mm512_unpackhi_epi64(row2, row3);
            let tmp4 = _mm512_unpacklo_epi64(row4, row5);
            let tmp5 = _mm512_unpackhi_epi64(row4, row5);
            let tmp6 = _mm512_unpacklo_epi64(row6, row7);
            let tmp7 = _mm512_unpackhi_epi64(row6, row7);

            let t0 = _mm512_shuffle_i64x2(tmp0, tmp2, 0x88);
            let t1 = _mm512_shuffle_i64x2(tmp0, tmp2, 0xDD);
            let t2 = _mm512_shuffle_i64x2(tmp1, tmp3, 0x88);
            let t3 = _mm512_shuffle_i64x2(tmp1, tmp3, 0xDD);
            let t4 = _mm512_shuffle_i64x2(tmp4, tmp6, 0x88);
            let t5 = _mm512_shuffle_i64x2(tmp4, tmp6, 0xDD);
            let t6 = _mm512_shuffle_i64x2(tmp5, tmp7, 0x88);
            let t7 = _mm512_shuffle_i64x2(tmp5, tmp7, 0xDD);

            let col0 = _mm512_shuffle_i64x2(t0, t4, 0x88);
            let col1 = _mm512_shuffle_i64x2(t2, t6, 0x88);
            let col2 = _mm512_shuffle_i64x2(t1, t5, 0x88);
            let col3 = _mm512_shuffle_i64x2(t3, t7, 0x88);
            let col4 = _mm512_shuffle_i64x2(t0, t4, 0xDD);
            let col5 = _mm512_shuffle_i64x2(t2, t6, 0xDD);
            let col6 = _mm512_shuffle_i64x2(t1, t5, 0xDD);
            let col7 = _mm512_shuffle_i64x2(t3, t7, 0xDD);

            // Store transposed block
            _mm512_storeu_si512(dst_ptr.add(i * num_rows + j) as *mut __m512i, col0);
            _mm512_storeu_si512(dst_ptr.add((i + 1) * num_rows + j) as *mut __m512i, col1);
            _mm512_storeu_si512(dst_ptr.add((i + 2) * num_rows + j) as *mut __m512i, col2);
            _mm512_storeu_si512(dst_ptr.add((i + 3) * num_rows + j) as *mut __m512i, col3);
            _mm512_storeu_si512(dst_ptr.add((i + 4) * num_rows + j) as *mut __m512i, col4);
            _mm512_storeu_si512(dst_ptr.add((i + 5) * num_rows + j) as *mut __m512i, col5);
            _mm512_storeu_si512(dst_ptr.add((i + 6) * num_rows + j) as *mut __m512i, col6);
            _mm512_storeu_si512(dst_ptr.add((i + 7) * num_rows + j) as *mut __m512i, col7);
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

pub unsafe fn mary_transpose_offset_1_blocked_8x8<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_8x8(
                fpolys.dat, res.dat,
                num_cols, num_rows,
                i_tile, i_end, j_tile, j_end
            );
        }
    }
}

unsafe fn transpose_tile_8x8(
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
    while i + 8 <= tile_cols {
        let global_i = i_start + i;

        let mut j = 0;
        while j + 8 <= tile_rows {
            let global_j = j_start + j;

            // Load 8x8 block
            let row0 = _mm512_loadu_si512(src_ptr.add(global_j * num_cols + global_i) as *const __m512i);
            let row1 = _mm512_loadu_si512(src_ptr.add((global_j + 1) * num_cols + global_i) as *const __m512i);
            let row2 = _mm512_loadu_si512(src_ptr.add((global_j + 2) * num_cols + global_i) as *const __m512i);
            let row3 = _mm512_loadu_si512(src_ptr.add((global_j + 3) * num_cols + global_i) as *const __m512i);
            let row4 = _mm512_loadu_si512(src_ptr.add((global_j + 4) * num_cols + global_i) as *const __m512i);
            let row5 = _mm512_loadu_si512(src_ptr.add((global_j + 5) * num_cols + global_i) as *const __m512i);
            let row6 = _mm512_loadu_si512(src_ptr.add((global_j + 6) * num_cols + global_i) as *const __m512i);
            let row7 = _mm512_loadu_si512(src_ptr.add((global_j + 7) * num_cols + global_i) as *const __m512i);

            // Transpose
            let tmp0 = _mm512_unpacklo_epi64(row0, row1);
            let tmp1 = _mm512_unpackhi_epi64(row0, row1);
            let tmp2 = _mm512_unpacklo_epi64(row2, row3);
            let tmp3 = _mm512_unpackhi_epi64(row2, row3);
            let tmp4 = _mm512_unpacklo_epi64(row4, row5);
            let tmp5 = _mm512_unpackhi_epi64(row4, row5);
            let tmp6 = _mm512_unpacklo_epi64(row6, row7);
            let tmp7 = _mm512_unpackhi_epi64(row6, row7);

            let t0 = _mm512_shuffle_i64x2(tmp0, tmp2, 0x88);
            let t1 = _mm512_shuffle_i64x2(tmp0, tmp2, 0xDD);
            let t2 = _mm512_shuffle_i64x2(tmp1, tmp3, 0x88);
            let t3 = _mm512_shuffle_i64x2(tmp1, tmp3, 0xDD);
            let t4 = _mm512_shuffle_i64x2(tmp4, tmp6, 0x88);
            let t5 = _mm512_shuffle_i64x2(tmp4, tmp6, 0xDD);
            let t6 = _mm512_shuffle_i64x2(tmp5, tmp7, 0x88);
            let t7 = _mm512_shuffle_i64x2(tmp5, tmp7, 0xDD);

            let col0 = _mm512_shuffle_i64x2(t0, t4, 0x88);
            let col1 = _mm512_shuffle_i64x2(t2, t6, 0x88);
            let col2 = _mm512_shuffle_i64x2(t1, t5, 0x88);
            let col3 = _mm512_shuffle_i64x2(t3, t7, 0x88);
            let col4 = _mm512_shuffle_i64x2(t0, t4, 0xDD);
            let col5 = _mm512_shuffle_i64x2(t2, t6, 0xDD);
            let col6 = _mm512_shuffle_i64x2(t1, t5, 0xDD);
            let col7 = _mm512_shuffle_i64x2(t3, t7, 0xDD);

            // Store
            _mm512_storeu_si512(dst_ptr.add(global_i * num_rows + global_j) as *mut __m512i, col0);
            _mm512_storeu_si512(dst_ptr.add((global_i + 1) * num_rows + global_j) as *mut __m512i, col1);
            _mm512_storeu_si512(dst_ptr.add((global_i + 2) * num_rows + global_j) as *mut __m512i, col2);
            _mm512_storeu_si512(dst_ptr.add((global_i + 3) * num_rows + global_j) as *mut __m512i, col3);
            _mm512_storeu_si512(dst_ptr.add((global_i + 4) * num_rows + global_j) as *mut __m512i, col4);
            _mm512_storeu_si512(dst_ptr.add((global_i + 5) * num_rows + global_j) as *mut __m512i, col5);
            _mm512_storeu_si512(dst_ptr.add((global_i + 6) * num_rows + global_j) as *mut __m512i, col6);
            _mm512_storeu_si512(dst_ptr.add((global_i + 7) * num_rows + global_j) as *mut __m512i, col7);

            j += 8;
        }

        // Remaining rows
        while j < tile_rows {
            let global_j = j_start + j;
            for k in 0..8 {
                *dst_ptr.add((global_i + k) * num_rows + global_j) =
                    *src_ptr.add(global_j * num_cols + global_i + k);
            }
            j += 1;
        }

        i += 8;
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

pub unsafe fn mary_transpose_offset_1_8x8_nt(
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

    const BLOCK: usize = 8;
    const PREFETCH_DISTANCE: usize = 2;

    let cols_blocked = (num_cols / BLOCK) * BLOCK;
    let rows_blocked = (num_rows / BLOCK) * BLOCK;

    for i in (0..cols_blocked).step_by(BLOCK) {
        for j in (0..rows_blocked).step_by(BLOCK) {
            // Prefetch future blocks
            if j + PREFETCH_DISTANCE * BLOCK < rows_blocked {
                for pf in 0..BLOCK {
                    _mm_prefetch(
                        src_ptr.add((j + PREFETCH_DISTANCE * BLOCK + pf) * num_cols + i) as *const i8,
                        _MM_HINT_T0
                    );
                }
            }

            // Load 8x8 block
            let row0 = _mm512_loadu_si512(src_ptr.add(j * num_cols + i) as *const __m512i);
            let row1 = _mm512_loadu_si512(src_ptr.add((j + 1) * num_cols + i) as *const __m512i);
            let row2 = _mm512_loadu_si512(src_ptr.add((j + 2) * num_cols + i) as *const __m512i);
            let row3 = _mm512_loadu_si512(src_ptr.add((j + 3) * num_cols + i) as *const __m512i);
            let row4 = _mm512_loadu_si512(src_ptr.add((j + 4) * num_cols + i) as *const __m512i);
            let row5 = _mm512_loadu_si512(src_ptr.add((j + 5) * num_cols + i) as *const __m512i);
            let row6 = _mm512_loadu_si512(src_ptr.add((j + 6) * num_cols + i) as *const __m512i);
            let row7 = _mm512_loadu_si512(src_ptr.add((j + 7) * num_cols + i) as *const __m512i);

            // Transpose 8x8
            let tmp0 = _mm512_unpacklo_epi64(row0, row1);
            let tmp1 = _mm512_unpackhi_epi64(row0, row1);
            let tmp2 = _mm512_unpacklo_epi64(row2, row3);
            let tmp3 = _mm512_unpackhi_epi64(row2, row3);
            let tmp4 = _mm512_unpacklo_epi64(row4, row5);
            let tmp5 = _mm512_unpackhi_epi64(row4, row5);
            let tmp6 = _mm512_unpacklo_epi64(row6, row7);
            let tmp7 = _mm512_unpackhi_epi64(row6, row7);

            let t0 = _mm512_shuffle_i64x2(tmp0, tmp2, 0x88);
            let t1 = _mm512_shuffle_i64x2(tmp0, tmp2, 0xDD);
            let t2 = _mm512_shuffle_i64x2(tmp1, tmp3, 0x88);
            let t3 = _mm512_shuffle_i64x2(tmp1, tmp3, 0xDD);
            let t4 = _mm512_shuffle_i64x2(tmp4, tmp6, 0x88);
            let t5 = _mm512_shuffle_i64x2(tmp4, tmp6, 0xDD);
            let t6 = _mm512_shuffle_i64x2(tmp5, tmp7, 0x88);
            let t7 = _mm512_shuffle_i64x2(tmp5, tmp7, 0xDD);

            let col0 = _mm512_shuffle_i64x2(t0, t4, 0x88);
            let col1 = _mm512_shuffle_i64x2(t2, t6, 0x88);
            let col2 = _mm512_shuffle_i64x2(t1, t5, 0x88);
            let col3 = _mm512_shuffle_i64x2(t3, t7, 0x88);
            let col4 = _mm512_shuffle_i64x2(t0, t4, 0xDD);
            let col5 = _mm512_shuffle_i64x2(t2, t6, 0xDD);
            let col6 = _mm512_shuffle_i64x2(t1, t5, 0xDD);
            let col7 = _mm512_shuffle_i64x2(t3, t7, 0xDD);

            // Non-temporal stores
            _mm512_stream_si512(dst_ptr.add(i * num_rows + j) as *mut __m512i, col0);
            _mm512_stream_si512(dst_ptr.add((i + 1) * num_rows + j) as *mut __m512i, col1);
            _mm512_stream_si512(dst_ptr.add((i + 2) * num_rows + j) as *mut __m512i, col2);
            _mm512_stream_si512(dst_ptr.add((i + 3) * num_rows + j) as *mut __m512i, col3);
            _mm512_stream_si512(dst_ptr.add((i + 4) * num_rows + j) as *mut __m512i, col4);
            _mm512_stream_si512(dst_ptr.add((i + 5) * num_rows + j) as *mut __m512i, col5);
            _mm512_stream_si512(dst_ptr.add((i + 6) * num_rows + j) as *mut __m512i, col6);
            _mm512_stream_si512(dst_ptr.add((i + 7) * num_rows + j) as *mut __m512i, col7);
        }

        _mm_sfence(); // Ensure stores complete before continuing

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

pub unsafe fn mary_transpose_offset_1_blocked_8x8_nt<const TILE_SIZE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let num_rows = fpolys.len as usize;
    let num_cols = fpolys.step as usize;

    for i_tile in (0..num_cols).step_by(TILE_SIZE) {
        let i_end = (i_tile + TILE_SIZE).min(num_cols);
        for j_tile in (0..num_rows).step_by(TILE_SIZE) {
            let j_end = (j_tile + TILE_SIZE).min(num_rows);

            transpose_tile_avx512_8x8_nt(
                fpolys.dat, res.dat,
                num_cols, num_rows,
                i_tile, i_end, j_tile, j_end
            );
        }
    }
}

unsafe fn transpose_tile_avx512_8x8_nt(
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

    const PREFETCH_DISTANCE: usize = 2;

    let mut i = 0;
    while i + 8 <= tile_cols {
        let global_i = i_start + i;

        let mut j = 0;
        while j + 8 <= tile_rows {
            let global_j = j_start + j;

            // Prefetch
            if j + PREFETCH_DISTANCE * 8 < tile_rows {
                for pf in 0..8 {
                    _mm_prefetch(
                        src_ptr.add((global_j + PREFETCH_DISTANCE * 8 + pf) * num_cols + global_i) as *const i8,
                        _MM_HINT_T0
                    );
                }
            }

            // Load 8x8 block
            let row0 = _mm512_loadu_si512(src_ptr.add(global_j * num_cols + global_i) as *const __m512i);
            let row1 = _mm512_loadu_si512(src_ptr.add((global_j + 1) * num_cols + global_i) as *const __m512i);
            let row2 = _mm512_loadu_si512(src_ptr.add((global_j + 2) * num_cols + global_i) as *const __m512i);
            let row3 = _mm512_loadu_si512(src_ptr.add((global_j + 3) * num_cols + global_i) as *const __m512i);
            let row4 = _mm512_loadu_si512(src_ptr.add((global_j + 4) * num_cols + global_i) as *const __m512i);
            let row5 = _mm512_loadu_si512(src_ptr.add((global_j + 5) * num_cols + global_i) as *const __m512i);
            let row6 = _mm512_loadu_si512(src_ptr.add((global_j + 6) * num_cols + global_i) as *const __m512i);
            let row7 = _mm512_loadu_si512(src_ptr.add((global_j + 7) * num_cols + global_i) as *const __m512i);

            // Transpose
            let tmp0 = _mm512_unpacklo_epi64(row0, row1);
            let tmp1 = _mm512_unpackhi_epi64(row0, row1);
            let tmp2 = _mm512_unpacklo_epi64(row2, row3);
            let tmp3 = _mm512_unpackhi_epi64(row2, row3);
            let tmp4 = _mm512_unpacklo_epi64(row4, row5);
            let tmp5 = _mm512_unpackhi_epi64(row4, row5);
            let tmp6 = _mm512_unpacklo_epi64(row6, row7);
            let tmp7 = _mm512_unpackhi_epi64(row6, row7);

            let t0 = _mm512_shuffle_i64x2(tmp0, tmp2, 0x88);
            let t1 = _mm512_shuffle_i64x2(tmp0, tmp2, 0xDD);
            let t2 = _mm512_shuffle_i64x2(tmp1, tmp3, 0x88);
            let t3 = _mm512_shuffle_i64x2(tmp1, tmp3, 0xDD);
            let t4 = _mm512_shuffle_i64x2(tmp4, tmp6, 0x88);
            let t5 = _mm512_shuffle_i64x2(tmp4, tmp6, 0xDD);
            let t6 = _mm512_shuffle_i64x2(tmp5, tmp7, 0x88);
            let t7 = _mm512_shuffle_i64x2(tmp5, tmp7, 0xDD);

            let col0 = _mm512_shuffle_i64x2(t0, t4, 0x88);
            let col1 = _mm512_shuffle_i64x2(t2, t6, 0x88);
            let col2 = _mm512_shuffle_i64x2(t1, t5, 0x88);
            let col3 = _mm512_shuffle_i64x2(t3, t7, 0x88);
            let col4 = _mm512_shuffle_i64x2(t0, t4, 0xDD);
            let col5 = _mm512_shuffle_i64x2(t2, t6, 0xDD);
            let col6 = _mm512_shuffle_i64x2(t1, t5, 0xDD);
            let col7 = _mm512_shuffle_i64x2(t3, t7, 0xDD);

            // Non-temporal stores
            _mm512_stream_si512(dst_ptr.add(global_i * num_rows + global_j) as *mut __m512i, col0);
            _mm512_stream_si512(dst_ptr.add((global_i + 1) * num_rows + global_j) as *mut __m512i, col1);
            _mm512_stream_si512(dst_ptr.add((global_i + 2) * num_rows + global_j) as *mut __m512i, col2);
            _mm512_stream_si512(dst_ptr.add((global_i + 3) * num_rows + global_j) as *mut __m512i, col3);
            _mm512_stream_si512(dst_ptr.add((global_i + 4) * num_rows + global_j) as *mut __m512i, col4);
            _mm512_stream_si512(dst_ptr.add((global_i + 5) * num_rows + global_j) as *mut __m512i, col5);
            _mm512_stream_si512(dst_ptr.add((global_i + 6) * num_rows + global_j) as *mut __m512i, col6);
            _mm512_stream_si512(dst_ptr.add((global_i + 7) * num_rows + global_j) as *mut __m512i, col7);

            j += 8;
        }

        _mm_sfence();

        // Remaining rows
        while j < tile_rows {
            let global_j = j_start + j;
            for k in 0..8 {
                *dst_ptr.add((global_i + k) * num_rows + global_j) =
                    *src_ptr.add(global_j * num_cols + global_i + k);
            }
            j += 1;
        }

        i += 8;
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