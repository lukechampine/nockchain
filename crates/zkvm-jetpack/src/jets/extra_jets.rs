use nockvm::interpreter::Context;
use nockvm::jets::bits::util as bits;
use nockvm::jets::util::slot;
use nockvm::jets::{JetErr, Result};
use nockvm::noun::{DirectAtom, Noun, D, T};

use tracing::log::*;

/*
 * +$  array  [len=@ dat=@ux]
 * +$  mary   [step=@ =array]
*/

pub fn transpose_jet(context: &mut Context, subject: Noun) -> Result {
    let sam = slot(subject, 6)?;

    let offset = slot(sam, 1)?.as_direct()?;
    trace!("offset: {}", offset.data());

    // ^-  mary
    let parent_core = slot(subject, 7).inspect_err(|v| warn!("Parent core not found {v:?}"))?;
    let parent_sample = slot(parent_core, 6).inspect_err(|v| warn!("Parent not found {v:?}"))?;
    trace!("paren: {parent_sample:?}",);

    // ^-  @
    let step = slot(parent_sample, 2)?
        .as_direct()
        .inspect_err(|e| warn!("Unable to get step {e:?}"))?;
    trace!("step: {step:?}");

    // ^-  array
    let array = slot(parent_sample, 3).inspect_err(|e| warn!("Unable to get array {e:?}"))?;
    trace!("array: {array:?}");

    // ^-  @
    let len = slot(array, 2)?
        .as_direct()
        .inspect_err(|e| warn!("len is not an atom {e:?}"))?;
    trace!("len: {len:?}");

    // ^-  @ux
    let data = slot(array, 3)?
        .as_atom()
        .inspect_err(|e| warn!("data is not an atom {e:?}"))?;
    trace!("data len: {}", data.as_ne_bytes().len());

    // Realistically, this data will not exceed 64-bits

    // =/  res-step  (mul len.array.ma offset)
    let res_step = len.data().checked_mul(offset.data()).ok_or(JetErr::Punt)?;

    // =/  res-len  (div step.ma offset)
    let res_len = step.data().checked_div(offset.data()).ok_or(JetErr::Punt)?;

    // =/  res=mary  [res-step [res-len (lsh [6 (mul res-step res-len)] 1)]]
    let mut res_data = bits::lsh(
        &mut context.stack,
        6,
        res_step as usize * res_len as usize,
        DirectAtom::new(1).unwrap().as_atom(),
    )?
    .as_atom()?;

    let res_array = T(&mut context.stack, &[D(res_len), res_data.as_noun()]);

    let res = T(&mut context.stack, &[D(res_step), res_array]);

    let res_bitslice = res_data.as_bitslice_mut();
    let src_bitslice = data.as_bitslice();
    let offset = offset.data() as usize;

    // =/  num-cols  res-len
    let num_cols = res_len as usize;
    // =/  num-rows  len.array.ma
    let num_rows = len.data() as usize;

    for i in 0..num_cols {
        for j in 0..num_rows {
            // =/  target-index  (add (mul i num-rows) j)
            let target_index = (i * num_rows) + j;
            // =/  source  (cut 6 [(mul offset (add (mul j num-cols) i)) offset] dat.array.ma)
            let src_block = offset * ((j * num_cols) + i);
            let si = src_block << 6;
            // m(dat.array (sew 6 [(mul offset target-index) offset source] dat.array.m))
            let dst_block = offset * target_index;
            let di = dst_block << 6;

            // Do the in-place copy (cut+sew).
            let bits = offset << 6;
            res_bitslice[di..(di + bits)].copy_from_bitslice(&src_bitslice[si..(si + bits)]);
        }
    }

    trace!(
        "Transposed from {}x{} to {res_step}x{res_len}",
        step.data(),
        len.data()
    );

    Ok(res)
}
