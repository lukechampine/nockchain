use array_concat::concat_arrays;
use either::Either;
use nockchain_math::belt::PRIME;
use nockchain_math::handle::{finalize_mary, new_handle_mut_mary};
use nockchain_math::mary::MarySlice;
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::structs::HoonList;
use nockvm::jets::util::BAIL_FAIL;
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::*;
use nockvm_macros::tas;
use zkvm_jetpack::form::mary::Mary;

use crate::seven::height_mary;

// ++  num-randomizers  1
const NUM_RANDOMIZERS: u64 = 1;

const BASIC_COL_NAMES_LEN: usize = 11;

fn header(stack: &mut NockStack) -> Noun {
    // ^-  header:table  ^~
    // TODO: Pull these automatically
    let r = [
        // :*  name:static:common
        D(tas!(b"compute")),
        //     p
        Atom::new(stack, PRIME).as_noun(),
        //     (lent basic-column-names:static:common)
        D(BASIC_COL_NAMES_LEN as _),
        //     (lent ext-column-names:static:common)
        D(55 * 3),
        //     (lent mega-ext-column-names:static:common)
        D(6 * 3),
        //     (lent column-names:static:common)
        D(BASIC_COL_NAMES_LEN as u64 + 55 * 3 + 6 * 3),
        //     num-randomizers
        D(NUM_RANDOMIZERS),
        // ==
    ];

    T(stack, &r)
}

fn op_map_get(op: usize) -> core::result::Result<[bool; 10], JetErr> {
    // ^-  (map @ op-flags)
    // %-  ~(gas by *(map @ op-flags))
    // :~  :-  0   [1 0 0 0 0 0 0 0 0 0]
    //     :-  1   [0 1 0 0 0 0 0 0 0 0]
    //     :-  2   [0 0 1 0 0 0 0 0 0 0]
    //     :-  3   [0 0 0 1 0 0 0 0 0 0]
    //     :-  4   [0 0 0 0 1 0 0 0 0 0]
    //     :-  5   [0 0 0 0 0 1 0 0 0 0]
    //     :-  6   [0 0 0 0 0 0 1 0 0 0]
    //     :-  7   [0 0 0 0 0 0 0 1 0 0]
    //     :-  8   [0 0 0 0 0 0 0 0 1 0]
    //     :-  9   [0 0 0 0 0 0 0 0 0 1]
    // ==
    if op >= 10 {
        return Err(BAIL_FAIL);
    }
    let mut ret = [false; 10];
    ret[op] = true;
    Ok(ret)
}

pub fn build(stack: &mut NockStack, ret: Noun) -> Result {
    // |=  fock-meta=fock-return
    let [queue, _, _, _] = ret.uncell()?;
    // ^-  table-mary
    // =/  queue=(list *)  queue.fock-meta
    let mut queue = HoonList::try_from(queue).ok().into_iter().flatten();
    // =|  rows=(list bpoly)
    let mut ret = Mary {
        len: 0,
        step: BASIC_COL_NAMES_LEN as _,
        dat: vec![],
    };
    // |-  ^-  table-mary
    // ?:  =(0 (lent queue))
    //   :-  header
    //   %-  zing-bpolys
    //   %-  flop
    //   :_  rows
    //   (init-bpoly [1 (reap (dec (lent basic-column-names:static:common)) 0)])
    // =|  row=row-data
    // =/  f      (snag 1 queue)
    queue.next();
    while let Some(f) = queue.next() {
        // =.  queue  (slag 3 queue)
        queue.next();
        // ?>  ?=(^ f)
        let f = f.as_cell()?;
        // =/  op     ?^(-.f %9 -.f)
        let op = match f.head().as_either_atom_cell() {
            Either::Left(a) => a.as_u64()? as usize,
            Either::Right(_) => 9,
        };
        // =/  ops    (~(got by op-map) op)
        let ops = op_map_get(op)?;
        // =.  rows
        //   :_  rows
        //   %-  init-bpoly
        //   :~  0  :: pad
        //       o0.ops
        //       o1.ops
        //       o2.ops
        //       o3.ops
        //       o4.ops
        //       o5.ops
        //       o6.ops
        //       o7.ops
        //       o8.ops
        //       o9.ops
        //   ==
        let ops: [u64; BASIC_COL_NAMES_LEN] = concat_arrays!([0], ops.map(|v| v as _));
        ret.dat.extend_from_slice(&ops);
        ret.len += 1;
        // =.  queue
        //   ?+  op  !!
        //     %0   queue
        //     %1   queue
        //     %2   (slag 2 queue)
        //     %3   (slag 1 queue)
        //     %4   (slag 1 queue)
        //     %5   (slag 2 queue)
        //     %6   (slag 3 queue)
        //     %7   (slag 1 queue)
        //     %8   (slag 2 queue)
        //     %9   (slag 2 queue)
        //   ==
        let skip = [0, 0, 2, 1, 1, 2, 3, 1, 2, 2];
        for _ in 0..skip[op] {
            queue.next();
        }
        // $
        // NOTE: so that we do (snag 1 queue) properly
        queue.next();
    }

    ret.dat.push(1);
    ret.dat.extend_from_slice(&[0; BASIC_COL_NAMES_LEN - 1]);
    ret.len += 1;

    let header = header(stack);

    let (ma, ma_handle) = new_handle_mut_mary(stack, ret.step as _, ret.len as _);
    ma_handle.dat.copy_from_slice(&ret.dat);
    let ma = finalize_mary(stack, ret.step as _, ret.len as _, ma);

    Ok(T(stack, &[header, ma]))
}

pub fn pad(stack: &mut NockStack, sam: Noun) -> Result {
    let [header, p] = sam.uncell()?;
    let Ok(p) = MarySlice::try_from(p) else {
        return Err(BAIL_FAIL);
    };
    let height = height_mary(p);
    let mut rows = Mary {
        step: p.step,
        len: p.len,
        dat: p.dat.to_vec(),
    };
    for _ in 0..(height - rows.len) {
        rows.dat.push(1);
        rows.dat.extend_from_slice(&[0; 10]);
        rows.len += 1;
    }
    let (ret_ma, h_ma) = new_handle_mut_mary(stack, rows.step as _, rows.len as _);
    h_ma.dat.copy_from_slice(&rows.dat);
    let ma = finalize_mary(stack, rows.step as _, rows.len as _, ret_ma);

    Ok(T(stack, &[header, ma]))
}
