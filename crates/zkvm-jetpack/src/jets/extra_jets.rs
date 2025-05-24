use std::sync::atomic::{AtomicBool, Ordering};

use crate::form::bpoly::{bp_hadamard, bpscal};
use crate::form::mary::MarySlice;
use crate::form::math::bpoly::bpadd;
use crate::form::math::tip5;
use crate::form::{bpow, PolySlice};
use crate::form::{poly::Poly, BPolySlice, Belt};
use crate::hand::handle::{finalize_poly, new_handle_mut_slice};
use crate::hand::structs::HoonList;
use crate::jets::bp_jets::bpoly_to_list;
use either::Either;
use nockvm::interpreter::Context;
use nockvm::jets::bits::util as bits;
use nockvm::jets::list::util as list;
use nockvm::jets::math::util as math;
use nockvm::jets::sort::util::gor;
use nockvm::jets::util::{self, slot};
use nockvm::jets::{util::BAIL_EXIT, JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::{Atom, Cell, DirectAtom, IndirectAtom, Noun, D, T, YES};
use nockvm::unifying_equality::unifying_equality;
use nockvm_macros::tas;

use tracing::log::*;

use super::bp_jets::init_bpoly;
use super::utils::jet_err;

macro_rules! jam_err {
    ($name:ident) => {{
        let jam_dir = concat!("./jams/", stringify!($name));
        JetErr::PuntJam(jam_dir)
    }};
}

macro_rules! jet_option {
    ($name:ident => : $b:expr) => { $b };
    // Run only only once, otherwise crash
    ($name:ident => 'run_once $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                static RUN: AtomicBool = AtomicBool::new(false);

                if !RUN.fetch_or(true, Ordering::Relaxed) {
                    $b
                } else {
                    jet_err()
                }
            }
        }
    };
    // Write jam files on crashes
    ($name:ident => 'jam_errs $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                if ret.is_err() {
                    Err(jam_err!($name))
                } else {
                    ret
                }
            }
        }
    };
    // Bypass crashes (reinterpret them)
    ($name:ident => 'punt_errs $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                if ret.is_err() {
                    Err(JetErr::Punt)
                } else {
                    ret
                }
            }
        }
    };
    // Jam invokations
    ($name:ident => 'jam $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let _ret = $b;
                Err(jam_err!($name))
            }
        }
    };
    // Create jam directory
    ($name:ident => 'create_jam_dir $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                let ret = $b;
                match ret {
                    Err(JetErr::PuntJam(d)) => {
                        let _ = std::fs::create_dir_all(d);
                        Err(JetErr::PuntJam(d))
                    }
                    v => v
                }
            }
        }
    };
    // Log invokations
    ($name:ident => 'log $($l:lifetime)*: $b:block) => {
        jet_option! {
            $name =>
            $($l)*:
            {
                eprintln!("Jet invoked: {}", stringify!($name));
                $b
            }
        }
    };
}

/// Extracts sample and calls the jet implementation.
///
/// This is so that we can have callable implementations for composing jets.
macro_rules! sam_jet {
    ($name:ident => $imp:ident $($l:lifetime)*$(,)?) => {
        pub fn $name(context: &mut Context, subject: Noun) -> Result {
            jet_option!($imp => $($l)*: {
                let sam = slot(subject, 6)?;
                $imp(&mut context.stack, sam)
            })
        }
    };
    ($name:ident => $imp:ident $($l:lifetime)*, $($rest:tt)*) => {
        sam_jet!($name => $imp $($l)*);
        sam_jet!($($rest)*);
    };
}

sam_jet! {
    hash_10_jet => hash_10,
    hash_belts_list_jet => hash_belts_list,
    hash_noun_varlen_jet => hash_noun_varlen,
    hash_varlen_jet => hash_varlen,
    hash_hashable_jet => hash_hashable,
    hash_ten_cell_jet => hash_ten_cell,
    leaf_sequence_jet => leaf_sequence,
    mp_substitute_mega_jet => mp_substitute_mega 'jam_errs 'create_jam_dir,// 'log 'punt_errs 'jam 'run_once 'create_jam_dir,
    mp_substitute_ultra_jet => mp_substitute_ultra, // 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    compute_composition_poly_jet => compute_composition_poly 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    bpdiv_jet => bpdiv 'jam 'create_jam_dir,
}

/*
 * +$  array  [len=@ dat=@ux]
 * +$  mary   [step=@ =array]
*/

pub fn step_mary(ma: Noun) -> Result {
    slot(ma, 2)
}

pub fn len_mary(ma: Noun) -> Result {
    slot(ma, 6)
}

pub fn array_mary(ma: Noun) -> Result {
    slot(ma, 3)
}

pub fn dat_mary(ma: Noun) -> Result {
    slot(ma, 7)
}

pub fn change_step(stack: &mut NockStack, new_step: Noun, ma: Noun) -> Result {
    // |=  [new-step=@]
    // ^-  mary

    let new_step = new_step.as_direct()?.data();

    let cur_step = slot(ma, 2)?.as_direct()?.data();
    let arr = slot(ma, 3)?;
    let cur_len = slot(arr, 2)?.as_direct()?.data();
    let data = slot(arr, 3)?.as_atom()?;

    // ?:  =(step.ma new-step)  ma
    if cur_step == new_step {
        return Ok(ma);
    }

    // ?>  =((mod (mul step.ma len.array.ma) new-step) 0)
    debug_assert_eq!((cur_step * cur_len) % new_step, 0);

    // :+  new-step
    //   (div (mul step.ma len.array.ma) new-step)
    // dat.array.ma
    Ok(T(
        stack,
        &[
            D(new_step),
            D((cur_step * cur_len) / new_step),
            data.as_noun(),
        ],
    ))
}

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

fn cut_direct(
    bloq: usize,
    start: usize,
    run: usize,
    atom: DirectAtom,
) -> core::result::Result<DirectAtom, JetErr> {
    if run == 0 {
        return Ok(D(0).as_direct()?);
    }

    if util::bite_to_word(bloq, run)? > 1 {
        return jet_err();
    }

    let direct = unsafe {
        let mut new_direct = D(0).as_direct()?;
        let new_slice = new_direct.as_bitslice_mut();
        util::chop(bloq, start, run, 0, new_slice, atom.as_bitslice())?;
        new_direct
    };

    Ok(direct)
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

pub fn reap(stack: &mut NockStack, size: usize, val: Noun) -> Result {
    produce_list(stack, 0, size, |_, _| val)
}

pub fn init_tip5_state(stack: &mut NockStack, domain: DirectAtom) -> Result {
    match domain.data() {
        // ^~((reap state-size 0))
        tas!(b"variable") => produce_list(stack, 0, STATE_SIZE, |_, _| D(0)),
        // ^~((weld (reap rate 0) (reap capacity (montify 1))))
        tas!(b"fixed") => {
            let reaped = reap(stack, RATE, D(0))?;
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

pub fn hash_10(stack: &mut NockStack, input: Noun) -> Result {
    // ::  +hash-10: hash list of 10 belts into a list of 5 belts
    // |=  input=(list belt)
    // ::  output length is 5
    // ^-  (list belt)

    // Verify that this list has length 10 and all elems are direct:
    // ?>  =((lent input) rate)
    // ?>  (levy input based)
    // FIXME: acc verify this

    // =.  input   (turn input montify)
    let input = scag_map(stack, usize::MAX, input, |stack, i| {
        montify(stack, i.as_atom()?).map(Atom::as_noun)
    })?;

    // =/  sponge  (init-tip5-state %fixed)
    let sponge = init_tip5_state(stack, DirectAtom::new(tas!(b"fixed"))?)?;

    // =.  sponge  (permutation (weld input (slag rate sponge)))
    let slagged = slag(RATE, sponge)?;
    let welded = list::weld(stack, input, slagged)?;
    let sponge = crate::jets::tip5_jets::permutation(stack, welded)?;

    // (turn (scag digest-length sponge) mont-reduction)
    let scagged = scag_map(stack, DIGEST_LENGTH, sponge, |stack, v| {
        mont_reduction(stack, v.as_atom()?).map(Atom::as_noun)
    })?;

    Ok(scagged)
}

pub fn hash_belts_list(stack: &mut NockStack, belts: Noun) -> Result {
    // |=  belts=(list belt)
    // ^-  noun-digest:tip5
    // =-  ?>  ?=(noun-digest -)  -
    // %-  list-to-tuple
    // (hash-varlen belts)
    let hashed = hash_varlen(stack, belts)?;
    list_to_tuple_inplace(hashed)
}

struct DP(Noun);

impl core::fmt::Debug for DP {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if let Ok(c) = self.0.as_cell() {
            write!(f, "{:?}", nockvm::noun::FullDebugCellDepth(&c, 3))
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

pub fn hash_noun_varlen(stack: &mut NockStack, n: Noun) -> Result {
    // ~/  %hash-noun-varlen
    // |=  n=*
    // ^-  noun-digest
    // =/  leaf=(list @)  (leaf-sequence:shape n)
    let leaf = leaf_sequence(stack, n)?;

    // =/  dyck=(list @)  (dyck:shape n)
    let dyck = dyck(stack, n)?;

    // =/  size  (lent leaf)
    let size = list::lent(leaf)?;

    // (hash-belts-list [size (weld leaf dyck)])
    let welded = list::weld(stack, leaf, dyck)?;
    let t = T(stack, &[D(size as u64), welded]);
    let r = hash_belts_list(stack, t)?;
    Ok(r)
}

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
        let hashed = hash_10(&mut context.stack, welded)
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

pub fn hash_hashable(stack: &mut NockStack, h: Noun) -> Result {
    // ~/  %hash-hashable
    // |=  h=hashable
    // ^-  noun-digest
    let h = h.as_cell()?;
    let ty = h.head().as_direct().map(|v| v.data());

    match ty {
        Ok(tas!(b"hash")) => {
            //trace!("hash");
            // ?:  ?=(%hash -.h)
            //   p.h
            return Ok(h.tail());
        }
        Ok(tas!(b"leaf")) => {
            //trace!("leaf");
            // ?:  ?=(%leaf -.h)
            //   (hash-noun-varlen p.h)
            return hash_noun_varlen(stack, h.tail());
        }
        Ok(tas!(b"list")) => {
            //trace!("list");
            // ?:  ?=(%list -.h)
            //   (hash-noun-varlen (turn p.h hash-hashable))
            let list = scag_map(stack, usize::MAX, h.tail(), |stack, v| {
                hash_hashable(stack, v)
            })?;
            return hash_noun_varlen(stack, list);
        }
        Ok(tas!(b"mary")) => {
            //trace!("mary");
            // ?:  ?=(%mary -.h)
            let ma = h.tail();

            //   %-  hash-hashable

            //   :-  leaf+step.p.h
            let step_ma = step_mary(ma).inspect_err(|e| trace!("step {e:?}"))?;
            let step = T(stack, &[D(tas!(b"leaf")), step_ma]);

            //   :-  leaf+len.array.p.h
            let len_ma = len_mary(ma).inspect_err(|e| trace!("len {e:?}"))?;
            let len = T(stack, &[D(tas!(b"leaf")), len_ma]);

            //   hash+(hash-belts-list (bpoly-to-list array:(~(change-step ave p.h) 1)))
            let ma = change_step(stack, D(1), ma).inspect_err(|e| trace!("change step {e:?}"))?;
            let arr = array_mary(ma).inspect_err(|e| trace!("arr {e:?}"))?;
            let l = bpoly_to_list(stack, arr).inspect_err(|e| trace!("bplist {e:?}"))?;
            let hash = hash_belts_list(stack, l).inspect_err(|e| trace!("hbl {e:?}"))?;
            let hash = T(stack, &[D(tas!(b"hash")), hash]);

            let f = T(stack, &[len, hash]);
            let f = T(stack, &[step, f]);

            return hash_hashable(stack, f);
        }
        _ => {
            //trace!("other");
            // %-  hash-ten-cell
            // [$(h p.h) $(h q.h)]
            let p = hash_hashable(stack, h.head())?;
            let q = hash_hashable(stack, h.tail())?;
            let c = Cell::new(stack, p, q);
            return hash_ten_cell(stack, c.as_noun());
        }
    }
}

/// Inplace modify list to a tuple.
/// If you wish to make this jettable, create a copy of the list, and then call this func.
pub fn list_to_tuple_inplace(n: Noun) -> Result {
    let mut prev = None;
    let mut cell = n.as_cell()?;

    loop {
        if let Ok(tail) = cell.tail().as_cell() {
            prev = Some(cell);
            cell = tail;
        } else {
            if let Some(mut prev) = prev {
                unsafe { (*prev.to_raw_pointer_mut()).tail = cell.head() };
                return Ok(n);
            } else {
                return Ok(cell.head());
            }
        }
    }
}

pub fn hash_ten_cell(stack: &mut NockStack, ten_cell: Noun) -> Result {
    // ~/  %hash-ten-cell
    // |=  =ten-cell
    // ^-  noun-digest
    // =-  ?>  ?=(noun-digest -)  -

    // NOTE: reversed order
    // %-  leaf-sequence:shape
    let seq = leaf_sequence(stack, ten_cell)?;

    // %-  hash-10
    let hash = hash_10(stack, seq)?;

    // %-  list-to-tuple
    let tup = list_to_tuple_inplace(hash)?;

    Ok(tup)
}

pub fn dyck(stack: &mut NockStack, t: Noun) -> Result {
    // ~/  %dyck
    // |=  t=*
    // %-  flop
    // ^-  (list @)
    // =|  vec=(list @)
    // |-
    // ?@  t  vec
    // $(t +.t, vec [1 $(t -.t, vec [0 vec])])
    // TODO: make this non-recursive
    fn recurse(stack: &mut NockStack, t: Noun, vec: Noun) -> Noun {
        let Ok(t) = t.as_cell() else {
            return vec;
        };
        let head_vec = Cell::new(stack, D(0), vec);
        let head_res = recurse(stack, t.head(), head_vec.as_noun());
        let tail_vec = Cell::new(stack, D(1), head_res);
        recurse(stack, t.tail(), tail_vec.as_noun())
    }
    let res = recurse(stack, t, D(0));
    list::flop(stack, res)
}

pub fn leaf_sequence(stack: &mut NockStack, mut t: Noun) -> Result {
    // ~/  %leaf-sequence
    // |=  t=*
    // %-  flop
    // ^-  (list @)
    // =|  vec=(list @)
    // |-
    // ?@  t  t^vec
    // $(t +.t, vec $(t -.t))

    // NOTE: Let's do this differently...
    // leaf-sequence constructs a flattened reversed list of all elems of t (as a list!), and then
    // reverses it. So, let's just flatten in-order...
    let ret = Cell::new(stack, D(0), D(0));
    let mut cur = ret;

    let mut prev = D(0);

    loop {
        match t.as_either_atom_cell() {
            Either::Left(a) => {
                // if t = atom:

                //   push t to cur.head
                unsafe { (*cur.to_raw_pointer_mut()).head = a.as_noun() };

                //   t = prev.pop_cell() else break
                let Ok(prev_t) = prev.as_cell() else { break };
                t = prev_t.head();
                prev = prev_t.tail();

                //   cur.tail = new_cell
                //   cur = new_cell
                let new_cell = Cell::new(stack, D(0), D(0));
                unsafe { (*cur.to_raw_pointer_mut()).tail = new_cell.as_noun() };
                cur = new_cell;
            }
            Either::Right(c) => {
                // else:

                //  prev.push_cell(cell.tail)
                prev = Cell::new(stack, c.tail(), prev).as_noun();

                //  t = cell.head
                t = c.head();
            }
        }
    }

    Ok(ret.as_noun())
}

fn pull_arg(inp: Noun) -> core::result::Result<(Noun, Noun), JetErr> {
    let c = inp.as_cell()?;
    Ok((c.head(), c.tail()))
}

fn pull_args<const N: usize>(mut inp: Noun) -> core::result::Result<[Noun; N], JetErr> {
    let mut cnt = 0;
    let ret = [(); N].map(|_| {
        cnt += 1;
        if cnt == N {
            Ok(inp)
        } else {
            let c = inp.as_cell()?;
            inp = c.tail();
            Ok(c.head())
        }
    });
    if let Some(Err(e)) = ret.iter().filter(|v| v.is_err()).next() {
        return Err(*e);
    }
    Ok(ret.map(|v| v.unwrap()))
}

fn tap_by(stack: &mut NockStack, a: Noun) -> Result {
    // =<  $
    // =+  b=`(list _?>(?=(^ a) n.a))`~

    fn recurse(stack: &mut NockStack, a: Noun, b: Noun) -> Result {
        // |.  ^+  b
        // ?~  a
        if a.as_direct().map(|v| v.data()) == Ok(0u64) {
            //   b
            Ok(b)
        } else {
            // $(a r.a, b [n.a $(a l.a)])
            let n = slot(a, 2)?;
            let l = slot(a, 6)?;
            let r = slot(a, 7)?;
            let left = recurse(stack, l, b)?;
            let b = Cell::new(stack, n, left);
            recurse(stack, r, b.as_noun())
        }
    }

    recurse(stack, a, D(0))
}

// Map walker
fn get_by(stack: &mut NockStack, a: Noun, mut b: Noun) -> Result {
    // ~/  %get
    // |*  b=*
    // =>  .(b `_?>(?=(^ a) p.n.a)`b)
    // |-  ^-  (unit _?>(?=(^ a) q.n.a))
    if a.as_direct().map(|v| v.data()) == Ok(0) {
        // ?~  a
        //   ~
        return Ok(a);
    }

    let n = slot(a, 2)?.as_cell()?;
    let mut p = n.head();
    let q = n.tail();

    if unsafe { unifying_equality(stack, &mut b, &mut p) } {
        // ?:  =(b p.n.a)
        //   (some q.n.a)
        Ok(Cell::new(stack, q, D(0)).as_noun())
    } else if gor(stack, b, p).as_direct().map(|v| v.data()) == Ok(0) {
        // ?:  (gor b p.n.a)
        //   $(a l.a)
        let l = slot(a, 6)?;
        get_by(stack, l, b)
    } else {
        // $(a r.a)
        let r = slot(a, 7)?;
        get_by(stack, r, b)
    }
}

fn got_by(stack: &mut NockStack, a: Noun, b: Noun) -> Result {
    let v = get_by(stack, a, b)?;
    Ok(v.as_cell()?.head())
}

fn got_by_val(stack: &mut NockStack, a: Noun, b: usize) -> Result {
    got_by(stack, a, D(b as u64))
}

fn noun_bpoly(stack: &mut NockStack, val: Noun) -> Result {
    let zpoly = Cell::new(stack, val, D(0));
    init_bpoly(stack, zpoly.as_noun())
}

fn zero_bpoly(stack: &mut NockStack) -> Result {
    noun_bpoly(stack, D(0))
}

// +$  mega-typ  ?(%var %rnd %dyn %con %com)
#[repr(u64)]
#[derive(Clone, Copy, Debug)]
enum MegaTyp {
    Con = 0,
    Var = 1,
    Rnd = 2,
    Dyn = 3,
    Com = 4,
}

impl MegaTyp {
    fn to_tas(self) -> u64 {
        match self {
            Self::Con => tas!(b"con"),
            Self::Var => tas!(b"var"),
            Self::Rnd => tas!(b"rnd"),
            Self::Dyn => tas!(b"dyn"),
            Self::Com => tas!(b"com"),
        }
    }
}

impl TryFrom<u64> for MegaTyp {
    type Error = ();

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        if value <= 4 {
            Ok(unsafe { core::mem::transmute(value) })
        } else {
            Err(())
        }
    }
}

// ::  bit length of type
// ++  typ-len  3
const TYP_LEN: usize = 3;
// ::  bit length of index
// ++  idx-len  10
const IDX_LEN: usize = 10;
// ::  bit length of exponent
// ++  exp-len  30
const EXP_LEN: usize = 30;

fn mega_typ(term: Noun) -> core::result::Result<MegaTyp, JetErr> {
    // ^-  mega-typ
    // ?+  (cut 0 [0 typ-len] term)  !!
    cut_direct(0, 0, TYP_LEN, term.as_direct()?)?
        .data()
        .try_into()
        .map_err(|_| jet_err().unwrap())
}

fn mega_idx(term: Noun) -> core::result::Result<usize, JetErr> {
    // ^-  @ud
    // (cut 0 [typ-len idx-len] term)
    Ok(cut_direct(0, TYP_LEN, IDX_LEN, term.as_direct()?)?.data() as _)
}

fn mega_exp(term: Noun) -> core::result::Result<u64, JetErr> {
    // ^-  @ud
    // (cut 0 [(add typ-len idx-len) exp-len] term)
    Ok(cut_direct(0, TYP_LEN + IDX_LEN, EXP_LEN, term.as_direct()?)?.data())
}

fn brek(ter: Noun) -> core::result::Result<(MegaTyp, usize, u64), JetErr> {
    //  |=  ter=mega-term
    //  ^-  [mega-typ @ @ud]
    //  :+  ~(typ mega ter)
    //    ~(idx mega ter)
    //  ~(exp mega ter)
    Ok((mega_typ(ter)?, mega_idx(ter)?, mega_exp(ter)?))
}

fn snag_bop(stack: &mut NockStack, k: Noun, i: usize) -> core::result::Result<u64, JetErr> {
    // NOTE: heree construct a mary of [step=1 dat=k],
    // and then snag i-th elem from the data.
    // If step wasn't 1, then it would be more complicated than just array index.
    let ma = Cell::new(stack, D(1), k);
    let Ok(ma) = MarySlice::try_from(ma.as_noun()) else {
        return jet_err();
    };
    Ok(ma.dat[i])
}

fn swag_bop(
    stack: &mut NockStack,
    k: Noun,
    i: usize,
    j: usize,
) -> core::result::Result<&[u64], JetErr> {
    // NOTE: see snag_bop
    let ma = Cell::new(stack, D(1), k);
    let Ok(ma) = MarySlice::try_from(ma.as_noun()) else {
        return jet_err();
    };

    if i >= ma.dat.len() {
        Ok(&[])
    } else {
        let r = ma.dat.split_at(i).1;
        Ok(&r[..core::cmp::min(j, r.len())])
    }
}

// Currently fails:
// Jet invoked: mp_substitute_ultra
// I (10:42:56) "mp-substitute-ultra - p=156.419.264; height=1.024; chal-map=1.419.372.573 dyns=924.507.158"
// Jet invoked: mp_substitute_ultra
// I (10:42:57) "mp-substitute-ultra - p=156.419.264; height=1.024; chal-map=1.419.372.573 dyns=924.507.158"
// I (10:43:00) /common/ztd/one.hoon:<[1.241 9].[1.241 26]>
// I (10:43:00) /common/ztd/one.hoon:<[1.241 5].[1.242 35]>
// I (10:43:00) /common/ztd/one.hoon:<[1.240 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.239 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.238 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.237 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.233 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.230 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.229 3].[1.253 5]>
// I (10:43:00) "Cannot divide by the zero polynomial."
// I (10:43:00) /common/ztd/one.hoon:<[1.228 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.225 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.222 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.221 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.220 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.219 3].[1.253 5]>
// I (10:43:00) /common/ztd/one.hoon:<[1.260 5].[1.260 16]>
// I (10:43:00) /common/ztd/one.hoon:<[1.260 3].[1.260 16]>
// I (10:43:00) /common/ztd/one.hoon:<[1.259 3].[1.260 16]>
// I (10:43:00) /common/ztd/eight.hoon:<[350 5].[366 7]>
// I (10:43:00) /common/ztd/eight.hoon:<[349 5].[366 7]>
// I (10:43:00) /common/ztd/eight.hoon:<[295 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[292 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[290 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[289 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[288 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[287 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[286 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[285 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[284 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[283 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[281 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[279 3].[368 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[278 3].[409 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[277 3].[409 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[274 3].[409 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[273 3].[409 5]>
// I (10:43:00) /common/ztd/eight.hoon:<[259 3].[259 35]>
// I (10:43:00) /common/ztd/eight.hoon:<[258 3].[259 35]>
// I (10:43:00) /common/stark/prover.hoon:<[318 5].[328 7]>
// I (10:43:00) /common/stark/prover.hoon:<[317 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[300 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[297 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[295 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[294 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[290 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[283 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[280 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[277 3].[549 28]>
// I (10:43:00) /common/stark/prover.hoon:<[276 3].[549 28]>
pub fn compute_composition_poly(stack: &mut NockStack, inp: Noun) -> Result {
    let args = pull_args(inp)?;
    let args = args.map(|v| mug(stack, v).data());

    let [omicrons, heights, tworow_trace_polys, constraint_map, constraint_counts, composition_chals, chal_map, dyn_map, is_extra] =
        args;

    eprintln!(
        "COMPUTE COMPOSITION POLY {:?} => {args:?}",
        mug(stack, inp).data()
    );
    Err(JetErr::Punt)
}

pub fn mp_substitute_ultra(stack: &mut NockStack, inp: Noun) -> Result {
    let args = pull_args(inp)?;
    let args = args.map(|v| mug(stack, v).data());

    let [mp, trace, max_height, chal_map, dyns] = args;

    eprintln!(
        "MP SUBSTITUTE ULTRA {:?} => {args:?}",
        mug(stack, inp).data()
    );
    Err(JetErr::Punt)
}

pub fn bpdiv(stack: &mut NockStack, inp: Noun) -> Result {
    Err(JetErr::Punt)
}

pub fn mp_substitute_mega(stack: &mut NockStack, inp: Noun) -> Result {
    // ::
    // ::  +mp-substitute-mega: Given a multipoly: sub in the chals, dyns, vars, and composition dependencies:
    // ::
    // ::  For vars, the trace polys: ~[p0(t) p1(t) ... ] are in eval form and we substitute pi(t) for xi.
    // ::
    // ::  The key insight is that multiplication is much faster on polynomials in eval form instead of
    // ::  coefficient form. Calling bpmul will do ntt's on the arguments and an ifft on the result
    // ::  over and over again. Instead we precompute the ntts for all the polynomials and those
    // ::  are the arguments to substitute. Since they're already in the correct form we just compute
    // ::  hadamard products on them, sum up all the terms, and do an ifft to get the result.
    // ::
    // ::  Another optimization is that the polynomials in eval form must be the length of the degree
    // ::  of the final product. Since the max degree of the constraints is 4 (this method has this
    // ::  constraint degree hardcoded for optimization purposes and must be changed by hand
    // ::  if the constraint degree changes), the vectors must be 4*n where n is the height.
    // ::
    // ++  mp-substitute-mega
    // ~/  %mp-substitute-mega
    // |=  [p=mp-mega trace-evals=bpoly height=@ chal-map=(map @ belt) dyns=bpoly com-map=(map @ bpoly)]
    let [p, trace_evals, height, chal_map, dyns, com_map] = pull_args(inp)?;
    // eprintln!("p={:?}", mug(stack, p).data());
    // eprintln!("trace_evals={:?}", mug(stack, trace_evals).data());
    // eprintln!("height={:?}", height);
    // eprintln!("chal_map={:?}", mug(stack, chal_map).data());
    // eprintln!("dyns={:?}", mug(stack, dyns).data());
    // eprintln!("com_map={:?}", mug(stack, com_map).data());

    // ^-  bpoly

    // %+  roll  ~(tap by p)
    let mut p_list = tap_by(stack, p)?;
    // eprintln!("plist={:?}", mug(stack, p_list));
    // |=  [[k=bpoly v=belt] acc=_zero-bpoly]
    let mut acc = zero_bpoly(stack)?;

    while let Ok(e) = p_list.as_cell() {
        p_list = e.tail();
        let [k, v] = pull_args(e.head())?;
        let v = v.as_atom()?.as_u64()?;
        // eprintln!(
        //     "rollling: k={:?}, v={:x}, acc={:?}",
        //     mug(stack, k),
        //     v,
        //     mug(stack, acc)
        // );

        // =/  [poly=bpoly len=@]  [trace-evals (mul 4 height)]
        let poly = trace_evals;
        let len = (height.as_atom()?.as_u64()? * 4) as usize;
        // eprintln!("trace-evals: poly={:?}, len={:?}", mug(stack, poly), len);

        // =/  ones=bpoly  (init-bpoly (reap len 1))
        let reaped = reap(stack, len, D(1))?;
        let ones = init_bpoly(stack, reaped)?;

        // ?:  =(v 0)  acc
        if v == 0 {
            continue;
        }

        // %+  bpadd  acc
        // %+  bpscal  v
        // %+  roll  (range len.k)
        let len_k = slot(k, 2)?.as_atom()?.as_u64()? as usize;
        // |=  [i=@ acc=_ones]
        let rolled = {
            let mut acc = ones;

            // ^-  bpoly
            for i in 0..len_k {
                // =/  ter  (~(snag bop k) i)
                let ter = snag_bop(stack, k, i)?;

                // =/  [typ=mega-typ:mp-to-mega idx=@ exp=@ud]
                //   (brek:mp-to-mega ter)
                let (typ, idx, exp) = brek(D(ter))?;

                // ?-  typ
                acc = match typ {
                    // %var
                    MegaTyp::Var => {
                        // =/  var=bpoly  (~(swag bop poly) (mul idx len) len)
                        let var = swag_bop(stack, poly, idx * len, len)?;
                        let var: BPolySlice = unsafe { core::mem::transmute(var) };
                        // %+  roll  (range exp)
                        // |=  [i=@ power=_acc]
                        for _ in 0..exp {
                            // (bp-hadamard power var)
                            acc = with_belts(stack, bp_hadamard, acc, var)?;
                        }
                        acc
                    }
                    // %rnd
                    MegaTyp::Rnd => {
                        // =/  rnd  (~(got by chal-map) idx)
                        let rnd = got_by_val(stack, chal_map, idx)?.as_atom()?.as_u64()?;
                        // (bpscal (bpow rnd exp) acc)
                        let powed = bpow(rnd, exp);
                        with_belts1(stack, bpscal, Belt(powed), acc)?
                    }
                    // %dyn
                    MegaTyp::Dyn => {
                        // =/  dyn  (~(snag bop dyns) idx)
                        let _dyn = snag_bop(stack, dyns, idx)?;
                        // (bpscal (bpow dyn exp) acc)
                        let powed = bpow(_dyn, exp);
                        with_belts1(stack, bpscal, Belt(powed), acc)?
                    }
                    // %con
                    MegaTyp::Con => {
                        // acc
                        acc
                    }
                    // %com
                    MegaTyp::Com => {
                        // =/  com=bpoly  (~(got by com-map) idx)
                        let com = got_by_val(stack, com_map, idx)?;
                        // %+  roll  (range exp)
                        // |=  [i=@ power=_acc]
                        for _ in 0..exp {
                            // (bp-hadamard power com)
                            acc = with_belts(stack, bp_hadamard, acc, com)?;
                        }
                        acc
                    }
                }
            }

            acc
        };
        // eprintln!("ROLLED {:?}", mug(stack, rolled));
        // NOTE: in reverse
        // :: %+  bpscal  v
        let res = with_belts1(stack, bpscal, Belt(v), rolled)?;
        // eprintln!("RES {:?}", mug(stack, res));
        // :: %+  bpadd  acc
        acc = with_belts(stack, bpadd, acc, res)?;
        // eprintln!("ADDED {:?}", mug(stack, acc));
    }

    Ok(acc)
}

fn with_belts1<'b, T>(
    stack: &mut NockStack,
    f: impl FnOnce(T, &[Belt], &mut [Belt]),
    bp: T,
    bq: impl TryInto<BPolySlice<'b>>,
) -> Result {
    let Ok(bq_poly) = bq.try_into() else {
        return jet_err();
    };
    let res_len = bq_poly.len();
    let (res, res_poly): (IndirectAtom, &mut [Belt]) = new_handle_mut_slice(stack, Some(res_len));

    f(bp, bq_poly.0, res_poly);

    let res_cell = finalize_poly(stack, Some(res_poly.len()), res);

    Ok(res_cell)
}

fn with_belts<'a, 'b>(
    stack: &mut NockStack,
    f: impl FnOnce(&[Belt], &[Belt], &mut [Belt]),
    bp: impl TryInto<BPolySlice<'a>>,
    bq: impl TryInto<BPolySlice<'b>>,
) -> Result {
    let (Ok(bp_poly), Ok(bq_poly)) = (bp.try_into(), bq.try_into()) else {
        return jet_err();
    };
    //assert_eq!(bp_poly.len(), bq_poly.len());
    let res_len = core::cmp::max(bp_poly.len(), bq_poly.len());
    let (res, res_poly): (IndirectAtom, &mut [Belt]) = new_handle_mut_slice(stack, Some(res_len));

    f(bp_poly.0, bq_poly.0, res_poly);

    let res_cell = finalize_poly(stack, Some(res_poly.len()), res);

    Ok(res_cell)
}

/*fn precompute_ntts(stack: &mut NockStack, inp: Noun) -> Result {
    // |=  [polys=mary height=@ ntt-len=@]
    let [polys, height, ntt_len] = pull_args(inp)?;
    // ^-  bpoly
    // %-  need
    // =/  new-len  (mul height ntt-len)
    let new_len = math::mul(stack, height, ntt_len)?;
    // %+  roll  (range len.array.polys)
    // |=  [i=@ acc=(unit bpoly)]
    // =/  p=bpoly  (~(snag-as-bpoly ave polys) i)
    // =/  fft=bpoly
    //   (bp-fft (~(zero-extend bop p) (sub new-len len.p)))
    // ?~  acc  (some fft)
    // (some (~(weld bop u.acc) fft))
}*/
