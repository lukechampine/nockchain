use crate::form::math::tip5;
use crate::hand::structs::HoonList;
use either::Either;
use nockvm::interpreter::Context;
use nockvm::jets::bits::util as bits;
use nockvm::jets::list::util as list;
use nockvm::jets::math::util as math;
use nockvm::jets::util::{self, slot};
use nockvm::jets::{util::BAIL_EXIT, JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, Cell, DirectAtom, IndirectAtom, Noun, D, T};
use nockvm_macros::tas;

use tracing::log::*;

/*
 * +$  array  [len=@ dat=@ux]
 * +$  mary   [step=@ =array]
*/

pub fn transpose_jet(context: &mut Context, subject: Noun) -> Result {
    let sam = slot(subject, 6)?;

    let offset = slot(sam, 1)?.as_direct()?;
    trace!("offset: {}", offset.data());

    // :: ^-  mary
    let parent_core = slot(subject, 7).inspect_err(|v| warn!("Parent core not found {v:?}"))?;
    let parent_sample = slot(parent_core, 6).inspect_err(|v| warn!("Parent not found {v:?}"))?;
    trace!("paren: {parent_sample:?}",);

    // :: ^-  @
    let step = slot(parent_sample, 2)?
        .as_direct()
        .inspect_err(|e| warn!("Unable to get step {e:?}"))?;
    trace!("step: {step:?}");

    // :: ^-  array
    let array = slot(parent_sample, 3).inspect_err(|e| warn!("Unable to get array {e:?}"))?;
    trace!("array: {array:?}");

    // :: ^-  @
    let len = slot(array, 2)?
        .as_direct()
        .inspect_err(|e| warn!("len is not an atom {e:?}"))?;
    trace!("len: {len:?}");

    // :: ^-  @ux
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
            let bits = offset << 6;

            // =/  target-index  (add (mul i num-rows) j)
            let target_index = (i * num_rows) + j;
            // =/  source  (cut 6 [(mul offset (add (mul j num-cols) i)) offset] dat.array.ma)
            let src_block = offset * ((j * num_cols) + i);
            let si = src_block << 6;
            let s = &src_bitslice[si..(si + bits)];
            // m(dat.array (sew 6 [(mul offset target-index) offset source] dat.array.m))
            let dst_block = offset * target_index;
            let di = dst_block << 6;
            let d = &mut res_bitslice[di..(di + bits)];

            // Do the sew (just, without intermediaries).
            d.copy_from_bitslice(s);
        }
    }

    trace!(
        "Transposed from {}x{} to {res_step}x{res_len}",
        step.data(),
        len.data()
    );

    Ok(res)
}

pub fn produce_list(
    stack: &mut NockStack,
    start: usize,
    end: usize,
    mut f: impl FnMut(&mut NockStack, usize) -> Noun,
) -> Result {
    let mut cur = D(0);

    for i in (start..end).rev() {
        let elem = f(stack, i);
        cur = Cell::new(stack, elem, cur).as_noun();
    }

    Ok(cur)
}

fn rsh(
    stack: &mut NockStack,
    bloq: usize,
    step: usize,
    a: Atom,
) -> core::result::Result<Atom, JetErr> {
    let len = bits::met(bloq, a);
    if step >= len {
        return Ok(Atom::new(stack, 0));
    }

    let new_size = util::bits_to_word(util::checked_sub(
        a.bit_size(),
        util::checked_left_shift(bloq, step)?,
    )?)?;
    unsafe {
        let (mut atom, dest) = IndirectAtom::new_raw_mut_bitslice(stack, new_size);
        util::chop(bloq, step, len - step, 0, dest, a.as_bitslice())?;
        Ok(atom.normalize_as_atom())
    }
}

fn cut(
    stack: &mut NockStack,
    bloq: usize,
    start: usize,
    run: usize,
    atom: Atom,
) -> core::result::Result<Atom, JetErr> {
    if run == 0 {
        return Ok(Atom::new(stack, 0));
    }

    let new_indirect = unsafe {
        let (mut new_indirect, new_slice) =
            IndirectAtom::new_raw_mut_bitslice(stack, util::bite_to_word(bloq, run)?);
        util::chop(bloq, start, run, 0, new_slice, atom.as_bitslice())?;
        new_indirect.normalize_as_atom()
    };

    Ok(new_indirect)
}

fn mont_reduction(stack: &mut NockStack, x: Atom) -> core::result::Result<Atom, JetErr> {
    // |=  x=melt
    // ^-  belt
    // ?>  (lth x rp)
    // debug_assert!(x < RP);

    // ++  p  0xffff.ffff.0000.0001
    let p = Atom::new(stack, 0xffffffff00000001);
    // ++  r  0x1.0000.0000.0000.0000
    // ++  r-mod-p  4.294.967.295
    // ++  r2  0xffff.fffe.0000.0001
    // ++  rp  0xffff.ffff.0000.0001.0000.0000.0000.0000
    // ++  g  7
    // ++  h  20.033.703.337

    // =/  x1  (cut 5 [1 1] x)
    let x1 = cut(stack, 5, 1, 1, x)?;

    // =/  x2  (rsh 6 x)
    let x2 = rsh(stack, 6, 1, x)?;

    // =/  c
    //   =/  x0  (end 5 x)
    let x0 = cut(stack, 5, 0, 1, x)?;

    //   (lsh 5 (add x0 x1))
    let c = math::add(stack, x0, x1);
    let c = bits::lsh(stack, 5, 1, c)?.as_atom()?;

    // =/  f   (rsh 6 c)
    let f = rsh(stack, 6, 1, c)?;

    // =/  d   (sub c (add x1 (mul f p)))
    let d = math::mul(stack, f, p);
    let d = math::add(stack, x1, d);
    let d = math::sub(stack, c, d)?;

    // ?:  (gte x2 d)
    if math::gte_b(stack, x2, d) {
        //   (sub x2 d)
        Ok(math::sub(stack, x2, d)?)
    } else {
        // (sub (add x2 p) d)
        let v = math::add(stack, x2, p);
        Ok(math::sub(stack, v, d)?)
    }
}

// ::  +montiply: computes a*b = (abr^{-1} mod p); note mul, not fmul: avoids mod p reduction!
fn montiply(stack: &mut NockStack, a: Atom, b: Atom) -> core::result::Result<Atom, JetErr> {
    // |:  [a=`melt`r-mod-p b=`melt`r-mod-p]
    // ^-  belt
    // ~+
    // ?>  ?&((based a) (based b))
    // FIXME: verify based
    let v = math::mul(stack, a, b);
    mont_reduction(stack, v)
}

// ::  +montify: transform to Montgomery space, i.e. compute x•r = xr mod p
fn montify(stack: &mut NockStack, x: Atom) -> core::result::Result<Atom, JetErr> {
    // ++  r2  0xffff.fffe.0000.0001
    let r2 = Atom::new(stack, 0xfffffffe00000001);

    // |=  x=belt
    // ^-  melt
    // ~+
    // (montiply x r2)
    montiply(stack, x, r2)
}

// FIXME: grab rate from arm
const RATE: usize = 10;
const STATE_SIZE: usize = 16;
const CAPACITY: usize = 6;

const DIGEST_LENGTH: usize = 5;

// ++  range
//   ~/  %range
//   |=  $@(@ ?(@ (pair @ @)))
//   ^-  (list @)
//   ?@  +<  ?~(+< ~ (gulf 0 (dec +<)))
//   (gulf p (dec q))
pub fn range_jet(context: &mut Context, subject: Noun) -> Result {
    let sam = slot(subject, 6)?;

    let (start, end) = match slot(sam, 1)?.as_either_atom_cell() {
        Either::Left(a) => (0, a.as_direct()?.data() as usize),
        Either::Right(c) => (
            c.head().as_direct()?.data() as usize,
            c.tail().as_direct()?.data() as usize,
        ),
    };

    produce_list(&mut context.stack, start, end, |_, i| D(i as u64))
}

pub fn init_tip5_state(stack: &mut NockStack, domain: DirectAtom) -> Result {
    match domain.data() {
        // ^~((reap state-size 0))
        tas!(b"variable") => produce_list(stack, 0, STATE_SIZE, |_, _| D(0)),
        // ^~((weld (reap rate 0) (reap capacity (montify 1))))
        tas!(b"fixed") => {
            let reaped = produce_list(stack, 0, RATE, |_, _| D(0))?;
            let one = Atom::new(stack, 1);
            let montified = montify(stack, one)?.as_noun();
            let montified = produce_list(stack, 0, CAPACITY, |_, _| montified)?;
            list::weld(stack, reaped, montified)
        }
        _ => Err(BAIL_EXIT),
    }
}

pub fn scag(stack: &mut NockStack, n: usize, list: Noun) -> Result {
    scag_map(stack, n, list, |_, v| Ok(v))
}

pub fn scag_map(
    stack: &mut NockStack,
    n: usize,
    list: Noun,
    mut f: impl FnMut(&mut NockStack, Noun) -> Result,
) -> Result {
    if n == 0 {
        return Ok(D(0));
    }

    let mut cell = list.as_cell()?;

    let head = f(stack, cell.head())?;
    let ret = Cell::new(stack, head, D(0));

    let mut cur = ret;

    for _ in 1..n {
        let Ok(tail) = cell.tail().as_cell() else {
            break;
        };

        cell = tail;

        let head = f(stack, cell.head())?;

        // Append new list entry
        let new_cell = Cell::new(stack, head, D(0));
        unsafe { (*cur.to_raw_pointer_mut()).tail = new_cell.as_noun() };
        cur = new_cell;
    }

    Ok(ret.as_noun())
}

pub fn slag(n: usize, list: Noun) -> Result {
    let mut cell = list.as_cell()?;

    for _ in 0..n {
        let Ok(tail) = cell.tail().as_cell() else {
            // TODO: should we crash here if i != n?
            return Ok(D(0));
        };
        cell = tail;
    }

    Ok(cell.as_noun())
}

pub fn hash_10(context: &mut Context, input: Noun) -> Result {
    // ::  +hash-10: hash list of 10 belts into a list of 5 belts
    // |=  input=(list belt)
    // ::  output length is 5
    // ^-  (list belt)

    // Verify that this list has length 10 and all elems are direct:
    // ?>  =((lent input) rate)
    // ?>  (levy input based)
    // FIXME: acc verify this

    // =.  input   (turn input montify)
    let input = scag_map(&mut context.stack, usize::MAX, input, |stack, i| {
        montify(stack, i.as_atom()?).map(Atom::as_noun)
    })?;

    // =/  sponge  (init-tip5-state %fixed)
    let sponge = init_tip5_state(&mut context.stack, DirectAtom::new(tas!(b"fixed"))?)?;

    // =.  sponge  (permutation (weld input (slag rate sponge)))
    let slagged = slag(RATE, sponge)?;
    let welded = list::weld(&mut context.stack, input, slagged)?;
    let sponge = crate::jets::tip5_jets::permutation(context, welded)?;

    // (turn (scag digest-length sponge) mont-reduction)
    let scagged = scag_map(&mut context.stack, DIGEST_LENGTH, sponge, |stack, v| {
        mont_reduction(stack, v.as_atom()?).map(Atom::as_noun)
    })?;

    Ok(scagged)
}

pub fn hash_10_jet(context: &mut Context, input: Noun) -> Result {
    let sam = slot(input, 6)?;
    hash_10(context, sam)
}

/*pub fn hash_belts_list(context: &mut Context, belts: Noun) -> Result {
    // |=  belts=(list belt)
    // ^-  noun-digest:tip5
    // =-  ?>  ?=(noun-digest -)  -
    // %-  list-to-tuple
    // (hash-varlen belts)
}*/

pub fn new_sponge(stack: &mut NockStack) -> Result {
    init_tip5_state(stack, DirectAtom::new(tas!(b"variable"))?)
}

pub fn absorb_sponge(
    stack: &mut NockStack,
    sponge: &mut [u64; tip5::STATE_SIZE],
    input: Noun,
) -> core::result::Result<(), JetErr> {
    // |=  input=(list belt)
    // ^+  +>.$
    // =*  rng  +>.$

    // |^
    // ::  assert that input is made of base field elements
    // ?>  (levy input based)

    // =/  [q=@ r=@]  (dvr (lent input) rate)
    let l = list::lent(input).inspect_err(|e| eprintln!("1: {e:?}"))?;
    let q = l / RATE;
    let r = l % RATE;

    // ::  pad input with ~[1 0 ... 0] to be a multiple of rate
    // =.  input  (weld input [1 (reap (dec (sub rate r)) 0)])
    let v = RATE - r - 1;
    let reapped = produce_list(stack, 0, v, |_, _| D(0)).inspect_err(|e| eprintln!("2: {e:?}"))?;
    let l = T(stack, &[D(1), reapped]);
    let input = list::weld(stack, input, l).inspect_err(|e| eprintln!("3: {e:?}"))?;

    // ::  bring input into montgomery space
    // =.  input  (turn input montify)
    let input = scag_map(stack, usize::MAX, input, |stack, v| {
        montify(stack, v.as_atom().inspect_err(|e| eprintln!("5: {e:?}"))?).map(Atom::as_noun)
    })
    .inspect_err(|e| eprintln!("4: {e:?}"))?;

    let mut input = HoonList::try_from(input).inspect_err(|e| eprintln!("5: {e:?}"))?;

    // |-
    // ?:  =(q 0)
    //   rng
    for _ in (0..=q).rev() {
        // =.  sponge  (absorb-rate (scag rate input))

        // ++  absorb-rate
        //   ?>  =((lent input) rate)
        let input_head = [(); RATE]
            .map(|_| input.next().unwrap())
            .map(|v| v.as_atom().unwrap())
            .map(|v| v.as_u64().unwrap());

        //   =.  sponge  (weld input (slag rate sponge))
        sponge[..RATE].copy_from_slice(&input_head);
        //   $:permute
        tip5::permute(sponge);
    }

    Ok(())
}

pub fn squeeze_sponge(stack: &mut NockStack, spo: &mut [u64; tip5::STATE_SIZE]) -> Result {
    // |.  ^+  [*(list belt) +.$]
    // =*  rng  +.$
    // ::  squeeze out the full rate and bring out of montgomery space
    // =/  output  (turn (scag rate sponge) mont-reduction)
    let mut list = D(0);
    for &e in spo[..RATE].iter().rev() {
        let a = Atom::new(stack, e);
        let a = mont_reduction(stack, a)?;
        let n = a.as_noun();
        list = T(stack, &[n, list]);
    }

    tip5::permute(spo);

    Ok(list)
}

pub fn hash_varlen(stack: &mut NockStack, input: Noun) -> Result {
    // |=  input=(list belt)
    // ^-  (list belt)
    // =/  spo  (new:sponge)
    let spo = new_sponge(stack)?;
    let mut spo = crate::jets::tip5_jets::hoon_list_to_sponge(spo)?;

    // =.  spo  (absorb:spo input)
    absorb_sponge(stack, &mut spo, input).inspect_err(|e| eprintln!("1: {e:?}"))?;

    // =^  output  spo
    //   (squeeze:spo)
    let output = squeeze_sponge(stack, &mut spo).inspect_err(|e| eprintln!("2: {e:?}"))?;

    // (scag digest-length output)
    scag(stack, DIGEST_LENGTH, output)
}

pub fn hash_varlen_jet(context: &mut Context, subject: Noun) -> Result {
    let sam = slot(subject, 6)?;
    hash_varlen(&mut context.stack, sam)
}

pub fn hash_pairs_jet(context: &mut Context, subject: Noun) -> Result {
    let sam = slot(subject, 6)?;

    // |=  lis=(list (list @))
    let lis = slot(sam, 1)?;
    trace!("lis: {lis:?} | {}", list::lent(lis)?);

    // If the list is empty, well, we can just reevaluate nock.
    let Ok(mut cell) = lis.as_cell() else {
        return Err(JetErr::Punt);
    };

    let ret = Cell::new(&mut context.stack, D(0), D(0));
    let mut cur = ret;

    // NOTE: this loop essentially takes the input list, and gets its pairs, reducing the size in
    // half. That's the point of `++  indices`.
    loop {
        // :: (snag b lis)
        let first = cell.head();

        match cell.tail().as_either_atom_cell() {
            Either::Left(_) => {
                // NOTE: here we are not hashing!
                // ?:  =(+(b) (lent lis))
                //   (snag b lis)
                unsafe { (*cur.to_raw_pointer_mut()).head = first };

                break;
            }
            Either::Right(tail) => cell = tail,
        }

        // :: (snag +(b) lis)
        let second = cell.head();

        // Other branch:
        // (hash-10:tip5 (weld (snag b lis) (snag +(b) lis)))
        // :: (weld <...>)
        let welded = list::weld(&mut context.stack, first, second)?;
        // hash-10:tip5
        let hashed = hash_10(context, welded)
            .inspect_err(|e| eprintln!("hash_10 failed: {e:?}"))
            .map_err(|_| JetErr::Punt)?;

        // Override the head
        unsafe { (*cur.to_raw_pointer_mut()).head = hashed };

        let Ok(tail) = cell.tail().as_cell() else {
            // We reached the end
            break;
        };
        cell = tail;

        // Append new list entry
        let new_cell = Cell::new(&mut context.stack, D(0), D(0));
        unsafe { (*cur.to_raw_pointer_mut()).tail = new_cell.as_noun() };
        cur = new_cell;
    }

    Ok(ret.as_noun())
}
