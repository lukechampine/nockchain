use crate::mary::{MarySlice, MarySliceMut};

#[inline(always)]
pub fn mary_transpose(fpolys: MarySlice, offset: usize, res: &mut MarySliceMut) {
    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step / offset;
    let num_rows = len;

    for i in 0..num_cols {
        for j in 0..num_rows {
            for k in 0..offset {
                res.dat[offset * (i * num_rows + j) + k] =
                    fpolys.dat[offset * (j * num_cols + i) + k];
            }
        }
    }
}

#[inline(always)]
pub fn mary_transpose_offset_1(fpolys: MarySlice, res: &mut MarySliceMut) {
    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    for i in 0..num_cols {
        for j in 0..num_rows {
            res.dat[i * num_rows + j] = fpolys.dat[j * num_cols + i];
        }
    }
}

#[inline(always)]
pub fn mary_transpose_offset_1_blocked<const TILE: usize>(
    fpolys: MarySlice,
    res: &mut MarySliceMut,
) {
    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    for ii in (0..num_cols).step_by(TILE) {
        for jj in (0..num_rows).step_by(TILE) {
            let i_end = (ii + TILE).min(num_cols);
            let j_end = (jj + TILE).min(num_rows);

            for i in ii..i_end {
                for j in jj..j_end {
                    // It would be great if we the bounds checks here could be optimized out, and we would not need unsafe.
                    // This improves performance by about 6%.
                    unsafe {
                        *res.dat.get_unchecked_mut(i * num_rows + j) =
                            *fpolys.dat.get_unchecked(j * num_cols + i);
                    }
                }
            }
        }
    }
}

#[inline(always)]
pub fn mary_transpose_offset_1_ptr(fpolys: MarySlice, res: &mut MarySliceMut) {
    let step = fpolys.step as usize;
    let len = fpolys.len as usize;
    let num_cols = step;
    let num_rows = len;

    unsafe {
        let src = fpolys.dat.as_ptr();
        let dst = res.dat.as_mut_ptr();

        for i in 0..num_cols {
            for j in 0..num_rows {
                *dst.add(i * num_rows + j) = *src.add(j * num_cols + i);
            }
        }
    }
}
