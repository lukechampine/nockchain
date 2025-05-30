use std::collections::BTreeMap;
use std::iter::{repeat, repeat_n};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::form::bpoly::{
    bp_hadamard, bp_hadamard_inplace, bpadd_, bpadd_in_place, bpdiv, bppow, bpscal, bpscal_inplace,
    bpsub_,
};
use crate::form::fext::{fadd, fadd_, fdiv_, finv_, fmul_, fneg, fneg_, fpow_};
use crate::form::mary::MarySlice;
use crate::form::math::poly::p_ntt;
use crate::form::math::tip5;
use crate::form::mega::{brek, MegaTyp};
use crate::form::{
    binv, bneg, bpow, BPolyVec, Element, FPolySlice, FPolySliceMut, FPolyVec, Felt, PolySlice,
    PolySliceMut, PolyVec,
};
use crate::form::{poly::Poly, BPolySlice, Belt};
use crate::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_felt, new_handle_mut_mary, new_handle_mut_slice,
};
use crate::hand::structs::{HoonList, HoonMap, HoonMapIter};
use crate::jets::bp_jets::bpoly_to_list;
use crate::jets::utils::det_err;
use crate::noun::noun_ext::NounExt;
use either::Either;
use nockvm::interpreter::Context;
use nockvm::jets::bits::util as bits;
use nockvm::jets::list::util::{self as list, lent};
use nockvm::jets::math::util as math;
use nockvm::jets::sort::util::gor;
use nockvm::jets::util::{self, slot};
use nockvm::jets::{util::BAIL_EXIT, JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::{Atom, Cell, DirectAtom, IndirectAtom, Noun, D, T, YES};
use nockvm::serialization::jam;
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
                println!("Jet invoked: {}", stringify!($name));
                $b
            }
        }
    };
}

/// Extracts sample and calls the jet implementation.
///
/// This is so that we can have callable implementations for composing jets.
macro_rules! sam_jet {
    ($name:ident => $imp:ident 'raw $($l:lifetime)*$(,)?) => {
        pub fn $name(context: &mut Context, subject: Noun) -> Result {
            jet_option!($imp => $($l)*: {
                $imp(context, subject)
            })
        }
    };
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
    hash_hashable_jet => hash_hashable,// 'jam 'create_jam_dir,
    hash_ten_cell_jet => hash_ten_cell,
    leaf_sequence_jet => leaf_sequence,
    mp_substitute_mega_jet => mp_substitute_mega, //'jam_errs 'create_jam_dir,// 'log 'punt_errs 'jam 'run_once 'create_jam_dir,
    mp_substitute_ultra_jet => mp_substitute_ultra, // 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    compute_composition_poly_jet => compute_composition_poly, // 'punt_errs 'run_once 'log 'jam 'create_jam_dir,
    compute_deep_jet => compute_deep,// 'jam 'create_jam_dir,
    fp_fft_jet => fp_fft_sam,
    fp_ifft_jet => fp_ifft_sam,
    fp_ntt_jet => fp_ntt_sam,
    do_init_mary_jet => do_init_mary,// 'jam 'create_jam_dir,
    // bpdiv_jet => bpdiv 'jam 'create_jam_dir,
    zero_extend_jet => zero_extend 'raw,// 'jam 'create_jam_dir,
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
    assert_eq!((cur_step * cur_len) % new_step, 0);

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

pub fn zero_extend(context: &mut Context, subject: Noun) -> Result {
    let parent_core = slot(subject, 7)?;
    let ma = slot(parent_core, 6)?;

    // |=  n=@
    // ^-  mary
    let n = slot(subject, 6)?.as_direct()?.data();

    let [step, len, dat] = pull_args(ma)?;
    let step = step.as_direct()?.data();
    let len = len.as_direct()?.data();
    let dat = dat.as_atom()?;
    let dat = dat.as_ne_bytes();

    assert!(
        dat.len() == (step * len * 8) as usize || dat.len() == (step * len + 1) as usize * 8,
        "Invalid mary atom: have step={step}, len={len}, but data length={} (bytes)",
        dat.len()
    );

    let dat_len = (step * len) as usize;
    let dat = unsafe { core::slice::from_raw_parts(dat.as_ptr() as *const u64, dat_len) };

    // :-  step.ma
    // :-  (add len.array.ma n)
    let new_alloc_len = (step * (len + n) + 1) as usize;
    // =/  i  0
    // =/  dat  dat.array.ma
    // |-
    // ?:  =(i n)
    //   dat
    // %_  $
    //   dat  dat.array:(~(snoc ave [step.ma (add len.array.ma i) dat]) (~(lift-elt mary-utils step.ma) 0))
    //   i    +(i)
    // ==
    let (out, buf) = unsafe { IndirectAtom::new_raw_mut(&mut context.stack, new_alloc_len) };
    let buf = unsafe { core::slice::from_raw_parts_mut(buf, new_alloc_len) };
    let (a, b) = buf.split_at_mut(dat.len());
    a.copy_from_slice(dat);
    b[..((step * n) as usize)].iter_mut().for_each(|v| *v = 0);
    b[(step * n) as usize] = 1;

    Ok(T(&mut context.stack, &[D(step), D(len + n), out.as_noun()]))
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
    // assert!(x < RP);

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
    let l = list::lent(input).inspect_err(|e| println!("1: {e:?}"))?;
    let q = l / RATE;
    let r = l % RATE;

    // ::  pad input with ~[1 0 ... 0] to be a multiple of rate
    // =.  input  (weld input [1 (reap (dec (sub rate r)) 0)])
    let v = RATE - r - 1;
    let reapped = produce_list(stack, 0, v, |_, _| D(0)).inspect_err(|e| println!("2: {e:?}"))?;
    let l = T(stack, &[D(1), reapped]);
    let input = list::weld(stack, input, l).inspect_err(|e| println!("3: {e:?}"))?;

    // ::  bring input into montgomery space
    // =.  input  (turn input montify)
    let input = scag_map(stack, usize::MAX, input, |stack, v| {
        montify(stack, v.as_atom().inspect_err(|e| println!("5: {e:?}"))?).map(Atom::as_noun)
    })
    .inspect_err(|e| println!("4: {e:?}"))?;

    let mut input = HoonList::try_from(input).inspect_err(|e| println!("5: {e:?}"))?;

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
    absorb_sponge(stack, &mut spo, input).inspect_err(|e| println!("1: {e:?}"))?;

    // =^  output  spo
    //   (squeeze:spo)
    let output = squeeze_sponge(stack, &mut spo).inspect_err(|e| println!("2: {e:?}"))?;

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
            .inspect_err(|e| println!("hash_10 failed: {e:?}"))
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

// ::  +mp-substitute-ultra
// ::
// ::  Handles substitution for %mega and %comp mp-ultra cases. If the multi-poly is a
// ::  single mp-mega constraint, we just call mp-substitute-mega on it. On the other hand
// ::  if it is a composition, we must first evaluate its dependencies, collating the
// ::  indexed results in a map. We then pass the map in as input when we substitute
// ::  the actual computation.
pub fn mp_substitute_ultra(stack: &mut NockStack, inp: Noun) -> Result {
    // ~/  %mp-substitute-ultra
    // |=  [p=mp-ultra trace-evals=bpoly height=@ chal-map=(map @ belt) dyns=bpoly]
    let [p, trace_evals, height, chal_map, dyns] = inp.uncell()?;

    let Ok(trace_evals) = BPolySlice::try_from(trace_evals) else {
        return jet_err();
    };

    let height = height.as_atom()?.as_u64()?;
    let chal_map = HoonMap::try_from(chal_map).ok();

    let Ok(dyns) = BPolySlice::try_from(dyns) else {
        return jet_err();
    };

    let ret = mp_substitute_ultra_impl(stack, p, trace_evals, height, chal_map, dyns)?;
    let mut ret = ret
        .into_iter()
        .map(|v| {
            let (res, res_poly): (IndirectAtom, &mut [Belt]) =
                new_handle_mut_slice(stack, Some(v.0.len()));
            res_poly.copy_from_slice(&v.0);
            let res_cell = finalize_poly(stack, Some(v.0.len()), res);
            res_cell
        })
        .collect::<Vec<_>>();
    ret.push(D(0));

    Ok(T(stack, &ret))
}

pub fn mp_substitute_ultra_impl(
    stack: &mut NockStack,
    p: Noun,
    trace_evals: BPolySlice,
    height: u64,
    chal_map: Option<HoonMap>,
    dyns: BPolySlice,
) -> core::result::Result<Vec<BPolyVec>, JetErr> {
    // ^-  (list bpoly)
    let [p_head, p_tail] = p.uncell()?;

    // ?-    -.p
    match p_head.as_direct()?.data() {
        // %mega
        tas!(b"mega") => {
            // :~  (mp-substitute-mega +.p trace-evals height chal-map dyns ~)
            // ==
            Ok(vec![mp_substitute_mega_impl(
                stack,
                p_tail,
                trace_evals,
                height,
                chal_map,
                dyns,
                &Default::default(),
            )?])
        }
        // %comp
        tas!(b"comp") => {
            let [dep, com] = p_tail.uncell()?;
            let dep = HoonList::try_from(dep)?;
            let com = HoonList::try_from(com)?;
            // =;  com-map=(map @ bpoly)
            let mut com_map = BTreeMap::new();
            // NOTE: swapped order (from =; to =/)
            // :: Materialize the dependencies and label them based on order
            // %+  roll
            //   (range (lent dep.p))
            for (i, mp) in dep.enumerate() {
                // |=  [i=@ acc=(map @ bpoly)]
                // =/  mp=mp-mega  (snag i dep.p)
                // %-  ~(put by acc)
                // :-  i
                // (mp-substitute-mega mp trace-evals height chal-map dyns ~)
                com_map.insert(
                    i as u64,
                    mp_substitute_mega_impl(
                        stack,
                        mp,
                        trace_evals,
                        height,
                        chal_map,
                        dyns,
                        &Default::default(),
                    )?,
                );
            }

            let mut ret = vec![];
            // %+  turn
            //   com.p
            for mp in com {
                // |=  mp=mp-mega
                // (mp-substitute-mega mp trace-evals height chal-map dyns com-map)
                ret.push(mp_substitute_mega_impl(
                    stack,
                    mp,
                    trace_evals,
                    height,
                    chal_map,
                    dyns,
                    &com_map,
                )?);
            }

            Ok(ret)
        }
        // ==
        _ => jet_err(),
    }
}

fn met_elt(elt: Atom) -> usize {
    // |=  =elt
    // ^-  @
    // (dec (max 2 (met 6 elt)))
    core::cmp::max(2, bits::met(6, elt)) - 1
}

fn init_mary(stack: &mut NockStack, poly: Noun) -> Result {
    // ~/  %init-mary
    // |=  poly=(list elt)
    // ^-  mary
    // ?~  poly  !!  :: can't return zero-mary because we can't figure out the step from ~
    let poly = poly.as_cell()?;
    // (do-init-mary (met-elt (head poly)) poly)
    let inp = Cell::new(
        stack,
        D(met_elt(poly.head().as_atom()?) as _),
        poly.as_noun(),
    );
    do_init_mary(stack, inp.as_noun())
}

fn lift_elt(stack: &mut NockStack, step: usize, a: Noun) -> Result {
    // ~/  %lift-elt
    // |=  a=@
    // ^-  elt
    // ?:(=(step 1) `@ux`a dat:(init-bpoly [a (reap (dec step) 0)]))
    if step == 1 {
        Ok(a)
    } else {
        let reaped = reap(stack, step - 1, D(0))?;
        let poly = T(stack, &[a, reaped]);
        let bp = init_bpoly(stack, poly)?;
        Ok(bp.as_cell()?.tail())
    }
}

fn zero_mary(stack: &mut NockStack, step: usize) -> Result {
    // ~+
    // ^-  mary
    // ?:  =(step 1)  [1 1 `@ux`0]
    if step == 1 {
        Ok(T(stack, &[D(1), D(1), D(0)]))
    } else {
        // (init-mary ~[(lift-elt 0)])
        let lifted = lift_elt(stack, step, D(0))?;
        let args = T(stack, &[lifted, D(0)]);
        init_mary(stack, args)
    }
}

fn do_init_mary(stack: &mut NockStack, inp: Noun) -> Result {
    // ~/  %do-init-mary
    // |=  [step=@ poly=(list elt)]
    let [step, poly] = pull_args(inp)?;
    let step = step.as_direct()?.data() as usize;

    // ^-  mary
    // ?:  =(~ poly)
    if poly.is_atom() {
        //   ~(zero-mary mary-utils step)
        zero_mary(stack, step)
    } else {
        // ?>  (lth (lent poly) (bex 32))
        // ?>  (levy poly |=(=elt &((~(fet mary-utils step) elt) =(step (met-elt elt)))))
        // :-  step
        // :-  (lent poly)
        let poly_len = lent(poly)?;
        // =/  high-bit  (lsh [0 (mul (bex 6) (mul step (lent poly)))] 1)
        let bstep = (1 << 6) * step * poly_len;
        let high_bit = bits::lsh(stack, 0, bstep, D(1).as_atom()?)?.as_atom()?;
        // (add (rep [6 step] poly) high-bit)
        let repped = bits::rep(stack, 6, step, poly)?;
        let added = math::add(stack, repped, high_bit);

        Ok(T(stack, &[D(step as _), D(poly_len as _), added.as_noun()]))
    }
}

pub fn init_fpoly(stack: &mut NockStack, poly: Noun) -> Result {
    // |=  poly=(list felt)
    // ^-  fpoly
    // ?~  poly  [0 (lift 0)]
    if poly.is_atom() {
        let (a, f) = new_handle_mut_felt(stack);
        *f = Felt::lift(Belt(0));
        Ok(Cell::new(stack, D(0), a.as_noun()).as_noun())
    } else {
        // array:(init-mary poly)
        let mary = init_mary(stack, poly)?;
        array_mary(mary)
    }
}

pub fn new_fpoly<'a>(d: &[Felt]) -> FPolyVec {
    copy_slice(PolySlice(d))
}

pub fn zero_fpoly<'a>() -> FPolyVec {
    // (init-fpoly ~[(lift 0)])
    new_fpoly(&[Felt::zero()])
}

pub fn id_fpoly<'a>() -> FPolyVec {
    // (init-fpoly ~[(lift 0) (lift 1)])
    new_fpoly(&[Felt::zero(), Felt::one()])
}

pub fn snag_mary(stack: &mut NockStack, ma: MarySlice, i: usize) -> Atom {
    // ~/  %snag
    // |=  i=@
    // ^-  elt
    // ?>  (lth i len.array.ma)
    // =/  res  (cut 6 [(mul i step.ma) step.ma] dat.array.ma)
    // NOTE: 6 is 64 bits, so... just index the mary
    // ?:  =(step.ma 1)  res
    let step = ma.step as usize;
    if step == 1 {
        Atom::new(stack, ma.dat[i])
    } else {
        let start = i * step;
        let res = &ma.dat[start..(start + step)];
        // =/  high-bit  (lsh [0 (mul (bex 6) step.ma)] 1)
        // (add high-bit res)
        let (out, dat) = unsafe { IndirectAtom::new_raw_mut(stack, step + 1) };
        let dat = unsafe { core::slice::from_raw_parts_mut(dat, step + 1) };
        dat[..step].copy_from_slice(res);
        dat[step] = 1;
        out.as_atom()
    }
}

pub fn snag_as_bpoly_mary<'a>(ma: MarySlice<'a>, i: usize) -> BPolySlice<'a> {
    // ~/  %snag-as-bpoly
    // |=  i=@
    // ^-  bpoly
    // :-  step.ma
    // =/  dat  (snag i)
    // ?:  =(step.ma 1)
    //   =/  high-bit  (lsh [0 (mul (bex 6) step.ma)] 1)
    //   (add high-bit dat)
    // dat
    let dat = ma.dat.split_at(i * (ma.step as usize)).1;
    let dat = dat.split_at(ma.step as usize).0;
    // SAFETY: Belt is a transparent wrapper for u64
    PolySlice(unsafe { core::mem::transmute::<&[u64], &[Belt]>(dat) })
}

pub fn bpoly_to_fpoly<'a>(bp: BPolySlice<'a>) -> FPolyVec {
    // ~/  %bpoly-to-fpoly
    // |=  bp=bpoly
    // ^-  fpoly
    // (lift-to-fpoly ~(to-poly bop bp))
    // NOTE: (to-poly bop bp) creates a mary of step 1, and calls mary-to-list
    // mary-to-list on a bpoly will always just give all atoms as a list, thus
    // we can just skip that, and call lift-to-fpoly using the input bp.
    // All in all, this function is just a lift_to_fpoly on a contiguous atom.
    PolyVec(bp.data().iter().copied().map(Felt::lift).collect())
}

pub fn fpadd<'a>(fp: FPolyVec, fq: FPolySlice) -> FPolyVec {
    // ~/  %fpadd
    // |:  [fp=`fpoly`zero-fpoly fq=`fpoly`zero-fpoly]
    // ^-  fpoly
    // ?>  &(!=(len.fp 0) !=(len.fq 0))
    // =/  p  ~(to-poly fop fp)
    // =/  q  ~(to-poly fop fq)
    // =/  lp  (lent p)
    // =/  lq  (lent q)
    // =/  m  (max lp lq)
    let m = core::cmp::max(fp.0.len(), fq.0.len());
    let mut fp = zeroextend_slice(fp, m, Felt::zero());
    //assert_eq!(m, fp.0.len());

    // =:  p  (weld p (reap (sub m lp) (lift 0)))
    //     q  (weld q (reap (sub m lq) (lift 0)))
    //   ==
    // %-  init-fpoly
    // (zip p q fadd)
    let zero_felt = &Felt::lift(Belt(0));
    let fq_iter = fq.0.iter().chain(repeat(zero_felt));
    for (p, q) in fp.0.iter_mut().zip(fq_iter) {
        *p = fadd_(p, q);
    }
    fp
}

pub fn fpneg(fp: &mut FPolySliceMut) {
    // |:  fp=`fpoly`zero-fpoly
    // ^-  fpoly
    // ?>  !=(len.fp 0)
    // ~+
    // =/  p  ~(to-poly fop fp)
    // %-  init-fpoly
    // (turn p fneg)
    for f in &mut fp.0[..] {
        *f = fneg_(f);
    }
}

fn copy_slice<'a, T: Copy>(a: PolySlice<T>) -> PolyVec<T> {
    PolyVec(a.0.to_vec())
}

fn copy_slice_extend_zero<T: Copy>(a: PolySlice<T>, n: usize, zero: T) -> PolyVec<T> {
    assert!(n >= a.0.len());
    PolyVec(
        a.0.iter()
            .copied()
            .chain(repeat_n(zero, n - a.0.len()))
            .collect(),
    )
}

fn alloc_slice<'a, T: Copy>(num: usize) -> PolyVec<T> {
    PolyVec(unsafe {
        let mut ret = Vec::with_capacity(num);
        ret.set_len(num);
        ret
    })
}

fn zeroextend_slice<'a, T: Copy>(mut a: PolyVec<T>, n: usize, zero: T) -> PolyVec<T> {
    if a.0.len() < n {
        a.0.resize(n, zero);
    }
    a
}

pub fn fpsub<'a>(p: FPolySlice, q: FPolySlice) -> FPolyVec {
    // ~/  %fpsub
    // |:  [p=`fpoly`zero-fpoly q=`fpoly`zero-fpoly]
    // ^-  fpoly
    // ~+
    // ?>  &(!=(len.p 0) !=(len.q 0))
    // (fpadd p (fpneg q))
    let mut neg = copy_slice_extend_zero(q, core::cmp::max(p.0.len(), q.0.len()), Felt::zero());
    fpneg(&mut (&mut neg).into());
    fpadd(neg, p)
}

pub fn fpscal<'a>(c: Felt, mut fp: FPolyVec) -> FPolyVec {
    // ~/  %fpscal
    // |:  [c=`felt`(lift 1) fp=`fpoly`one-fpoly]
    // ^-  fpoly
    // ~+
    // =/  p  ~(to-poly fop fp)
    // %-  init-fpoly
    // %+  turn
    //   p
    // (cury fmul c)
    fp.0.iter_mut().for_each(|v| *v = fmul_(v, &c));
    fp
}

fn fp_is_zero(p: FPolySlice) -> bool {
    // ~/  %fp-is-zero
    // |=  p=fpoly
    // ^-  ?
    // ~+
    // =.  p  (fpcan p)
    // |(=(len.p 0) =(p zero-fpoly))
    p.0.is_empty() || p.0[0] == Felt::zero()
}

pub fn fpdiv<'a>(
    stack: &mut NockStack,
    mut p: FPolyVec,
    q: FPolyVec,
) -> core::result::Result<FPolyVec, JetErr> {
    jam_to(stack, &p.0, "fpdiv-p");
    jam_to(stack, &q.0, "fpdiv-q");
    // ~/  %fpdiv
    // |:  [p=`fpoly`one-fpoly q=`fpoly`one-fpoly]
    // ^-  fpoly
    // ~+
    // ?>  &(!=(len.p 0) !=(len.q 0))
    assert!(!p.0.is_empty());
    assert!(!q.0.is_empty());
    // |^
    // =:  p  (fpcan p)
    //     q  (fpcan q)
    //   ==

    // ?:  (fp-is-zero q)
    if fp_is_zero((&q).into()) {
        // ~|  "Cannot divide by the zero polynomial!"
        error!("Cannot divide by the zero polynomial!");
        // !!
        return jet_err();
    }

    //println!("p={} q={}", vmug(stack, &p.0), vmug(stack, &q.0));

    // ?:  (fp-is-zero p)
    if fp_is_zero((&p).into()) {
        //println!("FP ZERO");
        // NOTE: p is never len 0, hence it's zero-fpoly
        // zero-fpoly
        //let p = PolySliceMut(&mut p.0[..1]);
        p.0.truncate(1);
        jam_to(stack, &p.0, "fpdiv-r");
        return Ok(p);
    }

    // =/  [c=felt f=fpoly]  (con-mon p)
    let (c, f) = con_mon(p);
    //println!("c={:?}", fat(stack, c));
    //println!("f={}", vmug(stack, &f.0));
    // =/  [d=felt g=fpoly]  (con-mon q)
    let (d, g) = con_mon(q);
    //println!("d={:?}", fat(stack, d));
    //println!("g={}", vmug(stack, &g.0));
    // =/  lead=felt  (fdiv c d)
    let lead = fdiv_(&c, &d);
    //println!("lead={:?}", fat(stack, lead));
    // NOTE: we do this before flop, because we flop in-place
    let df = fdegree((&f).into());
    // =/  rf=fpoly  ~(flop fop f)
    let mut rf = f;
    rf.0.reverse();
    //println!("rf={}", vmug(stack, &rf.0));
    // NOTE: we do this before flop, because we flop in-place
    let dg = fdegree((&g).into());
    // =/  rg=fpoly  ~(flop fop g)
    let mut rg = g;
    rg.0.reverse();
    //println!("rg={}", vmug(stack, &rg.0));
    //println!("df={}", df);
    //println!("dg={}", dg);
    // =/  df=@  (fdegree ~(to-poly fop f))
    // =/  dg=@  (fdegree ~(to-poly fop g))
    // ?:  (lth df dg)
    if df < dg {
        //   zero-fpoly
        let ret = zero_fpoly();
        jam_to(stack, &ret.0, "fpdiv-r");
        return Ok(ret);
    }
    // =/  dq=@  (sub df dg)
    let dq = df - dg;
    // %+  fpscal
    //   lead
    // %~  flop  fop
    // %.  +(dq)
    // %~  scag  fop
    // (fpmul (pinv-mod-x-to +(dq) rg) rf)
    let pinned = pinv_mod_x_to(stack, dq + 1, (&rg).into());
    //println!("pinved={}", vmug(stack, &pinned.0));
    let mulled = fpmul(stack, pinned, rf);
    //println!("mulled={}", vmug(stack, &mulled.0));
    let mut scagged = PolyVec(scag_ref(dq + 1, &mulled.0).to_vec());
    //println!("scagged={}", vmug(stack, &scagged.0));
    scagged.0.reverse();
    //println!("flopped={}", vmug(stack, &scagged.0));
    let ret = fpscal(lead, scagged);
    //println!("scalled={}", vmug(stack, &ret.0));
    jam_to(stack, &ret.0, "fpdiv-r");
    Ok(ret)
}

fn scag_mut<T>(a: usize, b: &mut [T]) -> &mut [T] {
    b.split_at_mut(core::cmp::min(a, b.len())).0
}

fn scag_ref<T>(a: usize, b: &[T]) -> &[T] {
    b.split_at(core::cmp::min(a, b.len())).0
}

fn slag_mut<T>(a: usize, b: &mut [T]) -> &mut [T] {
    b.split_at_mut(core::cmp::min(a, b.len())).1
}

fn slag_ref<T>(a: usize, b: &[T]) -> &[T] {
    b.split_at(core::cmp::min(a, b.len())).1
}

fn slag_vec<T>(a: usize, mut b: Vec<T>) -> Vec<T> {
    b.split_off(core::cmp::min(a, b.len()))
}

fn xeb(v: usize) -> usize {
    (usize::BITS - v.leading_zeros()) as usize
}

// ::  +pinv-mod-x-to: computes p^{-1} mod x^l
fn pinv_mod_x_to<'a>(stack: &mut NockStack, l: usize, p: FPolySlice) -> FPolyVec {
    jam_to(stack, p.0, "pmxt-p");
    jam_to2(stack, D(l as _), "pmxt-l");
    // |=  [l=@ p=fpoly]
    // ^-  fpoly
    // (~(scag fop (hensel-lift-inverse p (xeb l))) l)
    let mut lifted = hensel_lift_inverse(stack, p, xeb(l));
    lifted.0.truncate(l);
    jam_to(stack, &lifted.0, "pmxt-r");
    lifted
}

// ::
// ::  +hensel-lift-inverse: if p_0 = 1, compute p^{-1} mod x^{2^l} (l = level parameter below)
// ::
// ::    Given a(x) such that p(x)a(x) = 1 mod x^{2^i}, then a*p = 1 + x^{2^i}s(x) (see s below).
// ::    Letting t(x) = -a(x)*s(x) mod x^{2^i}, then p's inverse modulo x^{2^{i+1}} is
// ::    a(x) + x^{2^i}t(x)
fn hensel_lift_inverse<'a>(stack: &mut NockStack, p: FPolySlice, level: usize) -> FPolyVec {
    jam_to(stack, p.0, "hli-p");
    jam_to2(stack, D(level as _), "hli-l");
    // |=  [p=fpoly level=@]
    // ^-  fpoly
    // ~|  "Polynomial must have constant term equal to 1."
    // ?>  =(~(head fop p) (lift 1))
    //println!("p {}", vmug(stack, p.0));
    assert_eq!(p.0[0], Felt::one());
    // ::  since p_0 = 1, 1 is p's inverse mod x (x = x^{2^0})
    // =/  inv=fpoly  one-fpoly
    let mut inv = new_fpoly(&[Felt::one()]);
    // ::  have solution for level i, i.e. mod x^{2^i}, bootstrapping to next level
    // =/  i=@  0
    // |-
    // ?:  =(i level)
    //   inv
    for i in 0..level {
        // =/  bex-i=@  (bex i)
        let bex_i = 1 << i;
        //println!("bexed {bex_i}");
        // =/  s  (~(slag fop (fpmul p inv)) bex-i)
        let s = fpmul(stack, copy_slice(p), inv.clone());
        let s = PolyVec(slag_vec(bex_i, s.0));
        //println!("s {}", vmug(stack, s.0));
        // =/  t  (~(scag fop (fpmul (fpscal (lift (bneg 1)) inv) s)) bex-i)
        let t = fpscal(Felt::lift(Belt(bneg(1))), inv.clone());
        let mut t = fpmul(stack, t, s);
        //println!("t {}", vmug(stack, scag_ref(bex_i, &t.0)));
        //println!("l {bex_i}");
        // $(i +(i), inv (fpadd inv (pmul-by-x-to bex-i t)))
        // NOTE: t is the scagged from bex-i onwards, and pmul-by-x-to zero-extends t.
        // So, we can zero out the end, and achieve the same result.
        let len = scag_ref(bex_i, &t.0).len();
        let pos = t.0.len() - len;
        if pos != bex_i {
            t = copy_slice_extend_zero(PolySlice(&t.0[..len]), len + bex_i, Felt::zero());
        }
        t.0.copy_within(0..len, bex_i);
        t.0[..bex_i].iter_mut().for_each(|v| *v = Felt::zero());
        //println!("p {}", vmug(stack, &t.0));
        inv = fpadd(inv, (&t).into());
    }
    jam_to(stack, &inv.0, "hli-r");
    inv
}

// ::  pmul-by-x-to: multiply by x to the power l
fn pmul_by_x_to<'a>(stack: &mut NockStack, l: usize, p: FPolySlice) -> FPolyVec {
    jam_to(stack, p.0, "pbxt-p");
    jam_to2(stack, D(l as u64), "pbxt-l");
    // |=  [l=@ p=fpoly]
    // ^-  fpoly
    // %.  p
    // ~(weld fop (init-fpoly (reap l (lift 0))))
    let mut out = alloc_slice(l + p.0.len());
    let (a, b) = out.0.split_at_mut(l);
    a.iter_mut().for_each(|v| *v = Felt::zero());
    b.copy_from_slice(p.0);
    jam_to(stack, &out.0, "pbxt-r");
    out
}

// ::  +fpmul-naive: high school polynomial multiplication
fn fpmul_naive<'a>(fq: FPolyVec, fp: FPolyVec) -> FPolyVec {
    // ~/  %fpmul-naive
    // |=  [fp=fpoly fq=fpoly]
    // ^-  fpoly
    // ~+
    // =/  p  ~(to-poly fop fp)
    // =/  q  ~(to-poly fop fq)
    // %-  init-fpoly
    // ?:  ?|(=(~ p) =(~ q))
    //   ~
    if fp.0.is_empty() || fq.0.is_empty() {
        // NOTE: init-fpoly zero-inits null
        return new_fpoly(&mut [Felt::zero()]);
    }
    // =/  v=(list felt)
    //   %-  weld
    //   :_  p
    //   (reap (dec (lent q)) (lift 0))
    let extra = fq.0.len() - 1;
    let p_len = fp.len();
    let mut v = zeroextend_slice(fp, p_len + extra, Felt::zero());
    v.0.copy_within(0..p_len, extra);
    v.0[..extra].iter_mut().for_each(|v| *v = Felt::zero());
    let fp = ();
    let _ = fp;
    // =/  w=(list felt)  (flop q)
    let mut w = fq;
    let fq = ();
    let _ = fq;
    w.0.reverse();
    // =|  prod=poly
    let mut prod = v;
    // |-
    // ?~  v
    //   (flop prod)
    // %=  $
    for i in 0..prod.0.len() {
        let (_, v) = prod.0.split_at_mut(i);
        // v  t.v
        // ::
        //   prod
        // :_  prod
        // %.  [v w]
        // ::  computes a "dot product" (actually a bilinear form that just looks like
        // ::  one) of v and w by implicitly zero-extending if lengths unequal we
        // ::  don't actually zero-extend to save a constant time factor
        // |=  [v=(list felt) w=(list felt)]
        // ^-  felt
        // =/  dot=felt  (lift 0)
        let mut dot = Felt::zero();
        // |-
        // ?:  ?|(?=(~ v) ?=(~ w))
        //   dot
        for (v, w) in v.iter().zip(w.0.iter()) {
            // $(v t.v, w t.w, dot (fadd dot (fmul i.v i.w)))
            dot = fadd_(&dot, &fmul_(v, w));
        }
        // ==
        v[0] = dot;
    }
    if prod.0.is_empty() {
        return new_fpoly(&mut [Felt::zero()]);
    } else {
        // NOTE: flop part
        //prod.0.reverse();
        prod
    }
}

fn fp_ntt_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let [fp, root] = pull_args(sam)?;

    let Ok(p_poly) = FPolyVec::try_from(fp) else {
        return jet_err();
    };

    let returned_fpoly = p_ntt(p_poly.0, root.as_felt()?);
    let (res_atom, res_poly): (IndirectAtom, &mut [Felt]) =
        new_handle_mut_slice(stack, Some(returned_fpoly.len() as usize));

    res_poly.copy_from_slice(&returned_fpoly);

    let res_cell: Noun = finalize_poly(stack, Some(res_poly.len()), res_atom);

    Ok(res_cell)
}

fn fp_fft_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let Ok(p_poly) = FPolyVec::try_from(sam) else {
        return jet_err();
    };
    let returned_fpoly = fp_fft(p_poly)?;
    let (res_atom, res_poly): (IndirectAtom, &mut [Felt]) =
        new_handle_mut_slice(stack, Some(returned_fpoly.len() as usize));

    res_poly.copy_from_slice(&returned_fpoly.0);

    let res_cell: Noun = finalize_poly(stack, Some(res_poly.len()), res_atom);

    Ok(res_cell)
}

// ::  +fp-fft: Discrete Fourier Transform (DFT) with Fast Fourier Transform (FFT) algorithm
fn fp_fft(p: FPolyVec) -> core::result::Result<FPolyVec, JetErr> {
    // ~/  %fp-fft
    // |=  p=fpoly
    // ^-  fpoly
    // ~+
    // ~|  "fft: must have power-of-2-many coefficients."
    // ?>  =(0 (dis len.p (dec len.p)))
    assert_eq!(0, p.0.len() & (p.0.len() - 1));
    let root = Felt::ordered_root(p.0.len() as u64)?;
    Ok(PolyVec(p_ntt(p.0, &root)))
}

fn fp_ifft_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let Ok(p_poly) = FPolyVec::try_from(sam) else {
        return jet_err();
    };
    let returned_fpoly = fp_ifft(p_poly)?;
    let (res_atom, res_poly): (IndirectAtom, &mut [Felt]) =
        new_handle_mut_slice(stack, Some(returned_fpoly.len() as usize));

    res_poly.copy_from_slice(&returned_fpoly.0);

    let res_cell: Noun = finalize_poly(stack, Some(res_poly.len()), res_atom);

    Ok(res_cell)
}

fn bpcan(mut p: BPolyVec) -> BPolyVec {
    while let Some(v) = p.0.last() {
        if v.0 == 0 {
            p.0.pop();
        } else {
            break;
        }
    }
    if p.0.is_empty() {
        p.0.push(Belt(0));
    }
    p
}

// ::  +bp-ifft: Inverse DFT with FFT algorithm
fn bp_ifft<'a>(p: BPolyVec) -> core::result::Result<BPolyVec, JetErr> {
    // ~/  %bp-ifft
    // |=  p=bpoly
    // ^-  bpoly
    // ~+
    // ~|  "bp-ifft: must have power-of-2-many coefficients."
    // ?>  =((dis len.p (dec len.p)) 0)
    assert_eq!(0, p.0.len() & (p.0.len() - 1));
    // %+  bpscal  (binv len.p)
    // (bp-ntt p (binv (ordered-root len.p)))
    let binv_len = Belt(binv(p.0.len() as _));
    let Ok(or) = Belt(p.0.len() as _).ordered_root() else {
        return jet_err();
    };
    let root = Belt(binv(or.0));
    let mut ntt = p_ntt(p.0, &root);
    bpscal_inplace(binv_len, &mut ntt);
    Ok(PolyVec(ntt))
}

// ::  +fp-ifft: Inverse DFT with FFT algorithm
fn fp_ifft<'a>(p: FPolyVec) -> core::result::Result<FPolyVec, JetErr> {
    // ~/  %ifft
    // |=  p=fpoly
    // ^-  fpoly
    // ~+
    // ~|  "ifft: must have power-of-2-many coefficients."
    // ?>  =((dis len.p (dec len.p)) 0)
    assert_eq!(0, p.0.len() & (p.0.len() - 1));
    // %+  fpscal  (lift (binv len.p))
    // (fp-ntt p (lift (binv (ordered-root len.p))))
    let binv_len = Belt(binv(p.0.len() as _));
    let Ok(or) = Belt(p.0.len() as _).ordered_root() else {
        return jet_err();
    };
    let root = Felt::lift(Belt(binv(or.0)));
    let ntt = p_ntt(p.0, &root);
    Ok(fpscal(Felt::lift(binv_len), PolyVec(ntt)))
}

// ::  +fpmul-fast: polynomial multiplication with fft
fn fpmul_fast<'a>(fp: FPolyVec, fq: FPolyVec) -> FPolyVec {
    // ~/  %fpmul-fast
    // |=  [fp=fpoly fq=fpoly]
    // ^-  fpoly
    // ~+
    // =:  fp  (fpcan fp)
    //     fq  (fpcan fq)
    //   ==
    // ?:  ?|(=(fp zero-fpoly) =(fq zero-fpoly))
    if fp.0 == &[Felt::zero()] {
        return fp;
    } else if fq.0 == &[Felt::zero()] {
        //   zero-fpoly
        return zero_fpoly();
    };

    // =*  deg-p  len.fp
    let deg_p = fp.0.len();
    // =*  deg-q  len.fq
    let deg_q = fq.0.len();

    // =/  deg-prod  (bex (xeb (dec (add deg-p deg-q))))
    let deg_prod = 1 << xeb(deg_p + deg_q - 1);

    // %-  fpcan
    // %-  fp-ifft
    // %+  %~  zip  fop
    //     (fp-fft (~(zero-extend fop fp) (sub deg-prod deg-p)))
    let a = zeroextend_slice(fp, deg_prod, Felt::zero());
    let mut a = fp_fft(a).unwrap();
    //   (fp-fft (~(zero-extend fop fq) (sub deg-prod deg-q)))
    let b = zeroextend_slice(fq, deg_prod, Felt::zero());
    let mut b = fp_fft(b).unwrap();
    // fmul
    a.0.truncate(b.0.len());
    b.0.truncate(a.0.len());
    a.0.iter_mut()
        .zip(b.0.into_iter())
        .for_each(|(a, b)| *a = fmul_(a, &b));
    fp_ifft(a).unwrap()
}

// ::  +fpmul: polynomial multiplication
pub fn fpmul<'a>(stack: &mut NockStack, fp: FPolyVec, fq: FPolyVec) -> FPolyVec {
    jam_to(stack, &fp.0, "fpmul-fp");
    jam_to(stack, &fq.0, "fpmul-fq");
    // ~/  %fpmul
    // |:  [fp=`fpoly`one-fpoly fq=`fpoly`one-fpoly]
    // ^-  fpoly
    // ~+
    // ?:  |(=(len.fp 0) =(len.fq 0))
    //   (init-fpoly ~[(lift 0)])
    // =/  p  ~(to-poly fop fp)
    // =/  q  ~(to-poly fop fq)
    // ?:  (lth (add (fdegree p) (fdegree q)) 8)
    let degree = fdegree((&fp).into()) + fdegree((&fq).into());
    let ret = if degree < 8 {
        //   (fpmul-naive fp fq)
        fpmul_naive(fp, fq)
    } else {
        // (fpmul-fast fp fq)
        fpmul_fast(fp, fq)
    };
    jam_to(stack, &ret.0, "fpmul-r");
    ret
}

fn fcan<T: Element + Copy + PartialEq>(mut p: PolySlice<T>) -> PolySlice<T> {
    // |=  p=poly
    // ^-  poly
    // =.  p  (flop p)
    // |-
    // ?~  p
    //   ~
    // ?:  =(i.p (lift 0))
    //   $(p t.p)
    // (flop p)
    while p.0.last().map(Element::is_zero) == Some(true) {
        p = PolySlice(p.0.split_at(p.0.len() - 1).0)
    }
    p
}

fn fdegree<T: Element + Copy + PartialEq>(p: PolySlice<T>) -> usize {
    // |=  p=poly
    // ^-  @
    // =/  cp=poly  (fcan p)
    // ?~  cp  0
    // (dec (lent cp))
    fcan(p).0.len().saturating_sub(1)
}

// ::  con-mon: split p(x)!=0 uniquely into c*f(x) where c is constant f monic
fn con_mon(mut fp: FPolyVec) -> (Felt, FPolyVec) {
    // |=  fp=fpoly
    // ^-  [felt fpoly]
    // ~+
    // =.  fp  ~(flop fop (fpcan fp))
    fp.0.reverse();
    // ~|  "Cannot accept the zero polynomial!"
    // ?<  =(zero-fpoly fp)
    assert_ne!(fp.0, &[Felt::zero()]);
    // :-  ~(head fop fp)
    let head = fp.0[0];
    // %~  flop  fop
    // (fpscal (finv ~(head fop fp)) fp)
    let head_inv = finv_(&head);
    let mut fp = fpscal(head_inv, fp);
    fp.0.reverse();
    (head, fp)
}

pub fn weighted_linear_combo<'a>(
    stack: &mut NockStack,
    //cache: &mut HashMap<(FPolyVec, FPolyVec), core::result::Result<FPolyVec, JetErr>>,
    polys: &[FPolyVec],
    openings: FPolySlice<'a>,
    idx: usize,
    x_poly: FPolySlice<'a>,
    weights: FPolySlice<'a>,
) -> core::result::Result<(FPolyVec, usize), JetErr> {
    // |=  [polys=(list fpoly) openings=fpoly idx=@ x-poly=fpoly weights=fpoly]
    // ^-  [fpoly @]
    // =-  [acc num]
    let mut acc: FPolyVec = zero_fpoly();
    let mut num = idx;

    let id = id_fpoly();
    let id_x = fpsub((&id).into(), x_poly);

    // %+  roll  polys
    // |=  [poly=fpoly acc=_zero-fpoly num=_idx]
    for poly in polys {
        let poly: FPolySlice = poly.into();
        // :_  +(num)
        // %+  fpadd  acc
        // %+  fpscal  (~(snag fop weights) num)
        // %+  fpdiv
        //   (fpsub poly (fp-c (~(snag fop openings) num)))
        // (fpsub id-fpoly x-poly)
        // NOTE: id_x = (fpsub id-fpoly x-poly)
        let fpc = [openings.0[num]];
        /*println!(
            "id-x {} {} {}",
            vmug(stack, &id_x.0),
            vmug(stack, poly.0),
            vmug(stack, &fpc)
        );*/
        let fpc = PolySlice(&fpc);
        let res = fpsub(poly, fpc);
        //println!("res {}", vmug(stack, &res.0));
        /*let res = cache
        .entry((res, id_x.clone()))
        .or_insert_with_key(|(r, i)| fpdiv(stack, r.clone(), i.clone()))
        .clone()?;*/
        let res = fpdiv(stack, res, id_x.clone())?;
        //println!(
        //    "res {} {:?}",
        //    vmug(stack, &res.0),
        //    fat(stack, weights.0[num])
        //);
        let res = fpscal(weights.0[num], res);
        //println!("res {} {}", vmug(stack, &res.0), vmug(stack, &acc.0));
        acc = fpadd(acc, (&res).into());
        //println!("acc {}", vmug(stack, &acc.0));
        num += 1;
    }

    Ok((acc, num))
}

static mut JAM: bool = false;
//static mut JAMC: usize = 0;

fn jam_to2(stack: &mut NockStack, noun: Noun, p: &str) {
    if !unsafe { JAM } {
        return;
    }
    let dir = std::path::Path::new("jetjam");
    let p = dir.join(format!("{p}.jam"));
    std::fs::create_dir_all(dir);
    if !p.exists() {
        let out = jam(stack, noun);
        let b = out.as_ne_bytes();
        std::fs::write(p, b).unwrap()
    }
}

fn jam_to<T: Element + Copy>(stack: &mut NockStack, acc: &[T], p: &str) {
    if !unsafe { JAM } {
        return;
    }
    let dir = std::path::Path::new("jetjam");
    let f = dir.join(format!("{p}.jam"));
    if !f.exists() {
        let (res, res_poly): (IndirectAtom, &mut [T]) =
            new_handle_mut_slice(stack, Some(acc.len()));
        res_poly.copy_from_slice(acc);
        //println!("RES {acc:?} {res:?}");
        let res_cell = finalize_poly(stack, Some(acc.len()), res);
        jam_to2(stack, res_cell, p)
    }
}

/*fn enable_jam() {
    unsafe {
        JAMC += 1;
        if JAMC >= 2 {
            //println!("ENABLE JAM");
            JAM = true;
        }
    };
}*/
//fn jam_to2(_: &mut NockStack, _: Noun, _: &str) {}
//fn jam_to<T: Element + Copy>(_: &mut NockStack, _: &[T], _: &str) {}

fn vprint<T: Element + Copy>(stack: &mut NockStack, acc: &[T]) {
    let (res, res_poly): (IndirectAtom, &mut [T]) = new_handle_mut_slice(stack, Some(acc.len()));
    res_poly.copy_from_slice(acc);
    //println!("RES {acc:?} {res:?}");
    let res_cell = finalize_poly(stack, Some(acc.len()), res);
    println!("RC: {:?}", DP(res_cell))
    //mug(stack, res_cell).data()
}

fn vmug<T: Element + Copy>(stack: &mut NockStack, acc: &[T]) -> u64 {
    let (res, res_poly): (IndirectAtom, &mut [T]) = new_handle_mut_slice(stack, Some(acc.len()));
    res_poly.copy_from_slice(acc);
    //println!("RES {acc:?} {res:?}");
    let res_cell = finalize_poly(stack, Some(acc.len()), res);
    mug(stack, res_cell).data()
}

fn mmug(stack: &mut NockStack, m: &MarySlice) -> u64 {
    let (res, res_ma) = new_handle_mut_mary(stack, m.step as usize, m.len as usize);
    res_ma.dat.copy_from_slice(m.dat);
    let res_ma = finalize_mary(stack, m.step as usize, m.len as usize, res);
    mug(stack, res_ma).data()
}

fn fat(stack: &mut NockStack, f: Felt) -> IndirectAtom {
    let (res, res_felt): (IndirectAtom, &mut Felt) = new_handle_mut_felt(stack);
    *res_felt = f;
    res
}

pub fn compute_deep(stack: &mut NockStack, inp: Noun) -> Result {
    // ~/  %compute-deep
    // |=  $:  trace-polys=(list mary)
    //         trace-openings=fpoly
    //         composition-pieces=(list fpoly)
    //         composition-piece-openings=fpoly
    //         weights=fpoly
    //         omicrons=fpoly
    //         deep-challenge=felt
    //         comp-eval-point=felt
    //     ==
    // |^  ^-  fpoly
    let [trace_polys, trace_openings, composition_pieces, composition_piece_openings, weights, omicrons, deep_challenge, comp_eval_point] =
        pull_args(inp)?;

    // Convert nouns to appropriate types
    let trace_polys = HoonList::try_from(trace_polys)?
        .into_iter()
        .map(|x| MarySlice::try_from(x))
        .collect::<core::result::Result<Vec<_>, _>>()
        .or_else(|_| jet_err())?;

    let Ok(trace_openings) = FPolySlice::try_from(trace_openings) else {
        debug!("trace_openings is not a valid FPolySlice");
        return jet_err();
    };

    let composition_pieces = HoonList::try_from(composition_pieces)?
        .into_iter()
        .map(|x| FPolySlice::try_from(x).map(|v| PolyVec(v.0.to_vec())))
        .collect::<core::result::Result<Vec<_>, _>>()
        .or_else(|_| {
            debug!("composition_pieces contain invalid FPolySlice");
            jet_err()
        })?;

    let Ok(composition_piece_openings) = FPolySlice::try_from(composition_piece_openings) else {
        debug!("composition_piece_openings is not a valid FPolySlice");
        return jet_err();
    };

    let Ok(weights) = FPolySlice::try_from(weights) else {
        debug!("weights is not a valid FPolySlice");
        return jet_err();
    };

    let Ok(omicrons) = FPolySlice::try_from(omicrons) else {
        debug!("omicrons is not a valid FPolySlice");
        return jet_err();
    };

    let deep_challenge = deep_challenge.as_felt()?;
    let comp_eval_point = comp_eval_point.as_felt()?;

    /*println!(
        "COMPUTE DEEP: tp={} to={} cp={} cpo={} w={} o={} dc={deep_challenge:?} cep={comp_eval_point:?}",
        trace_polys.len(),
        trace_openings.0.len(),
        composition_pieces.len(),
        composition_piece_openings.0.len(),
        weights.0.len(),
        omicrons.0.len()
    );*/

    let mut acc = zero_fpoly();
    let mut num = 0usize;

    //let mut cache = Default::default();

    for (o, point) in [deep_challenge, comp_eval_point]
        .iter()
        .copied()
        .enumerate()
    {
        let fpc_point = new_fpoly(&[*point]);
        //println!("POINT {o} @ acc={}", vmug(stack, &acc.0));
        // |^  ^-  fpoly
        // =/  [acc=fpoly num=@]
        //   %^  zip-roll  (range (lent trace-polys))  trace-polys
        //   |=  [[i=@ p=mary] acc=_zero-fpoly num=@]
        for (i, &p) in trace_polys.iter().enumerate() {
            //println!("POLY {o}.{i} {} {}", vmug(stack, &acc.0), mmug(stack, &p));
            // =/  lis=(list fpoly)
            //   %+  turn  (range len.array.p)
            //   |=  i=@
            //   (bpoly-to-fpoly (~(snag-as-bpoly ave p) i))
            let mut lis = Vec::with_capacity(p.len as usize);
            for i in 0..p.len {
                let bp = snag_as_bpoly_mary(p, i as usize);
                let fp = bpoly_to_fpoly(bp);
                lis.push(fp);
            }

            // =/  omicron  (~(snag fop omicrons) i)
            let omicron = omicrons.0[i];
            //println!("OMICRON {:?}", fat(stack, omicron));

            // =/  [first-row=fpoly num=@]    :: first row:  f(x)-f(Z)/x-Z
            //   %-  weighted-linear-combo
            //   :*  lis
            //       trace-openings
            //       num
            //       (fp-c deep-challenge)
            //       weights
            //   ==
            let (first_row, new_num) = weighted_linear_combo(
                stack,
                &lis,
                trace_openings,
                num,
                (&fpc_point).into(),
                weights,
            )?;
            //println!("FIRST-ROW {}", vmug(stack, &first_row.0));

            // =/  [second-row=fpoly num=@]   :: second row:  f(x)-f(gZ)/x-gZ
            //   %-  weighted-linear-combo
            //   :*  lis
            //       trace-openings
            //       num
            //       (fp-c (fmul omicron deep-challenge))
            //       weights
            //   ==
            let point_omi_dc = new_fpoly(&[fmul_(&omicron, point)]);
            let (second_row, new_num) = weighted_linear_combo(
                stack,
                &lis,
                trace_openings,
                new_num,
                (&point_omi_dc).into(),
                weights,
            )?;
            //println!("SECOND-ROW {}", vmug(stack, &second_row.0));

            // :_  num
            num = new_num;
            // :(fpadd acc first-row second-row)
            acc = fpadd(acc, (&first_row).into());
            acc = fpadd(acc, (&second_row).into());
        }
    }

    // ::
    // ::  do the same thing for the second composition poly evals
    // =/  [acc=fpoly num=@]
    //   %^  zip-roll  (range (lent trace-polys))  trace-polys
    //   |=  [[i=@ p=mary] acc=_acc num=_num]
    //   =/  lis=(list fpoly)
    //     %+  turn  (range len.array.p)
    //     |=  i=@
    //     (bpoly-to-fpoly (~(snag-as-bpoly ave p) i))
    //   =/  omicron  (~(snag fop omicrons) i)
    //   ::  add new composition poly
    //   =/  [new-first-row=fpoly num=@]    :: first row:  f(x)-f(Z)/x-Z
    //     %-  weighted-linear-combo
    //     :*  lis
    //         trace-openings
    //         num
    //         (fp-c comp-eval-point)
    //         weights
    //     ==
    //   ::  second row
    //   =/  [new-second-row=fpoly num=@]   :: second row:  f(x)-f(gZ)/x-gZ
    //     %-  weighted-linear-combo
    //     :*  lis
    //         trace-openings
    //         num
    //         (fp-c (fmul omicron comp-eval-point))
    //         weights
    //     ==
    //   :_  num
    //   :(fpadd acc new-first-row new-second-row)
    // ::
    // =/  [pieces=fpoly @]
    //   %-  weighted-linear-combo
    //   :*  composition-pieces
    //       composition-piece-openings
    //       0
    //       (fp-c (fpow deep-challenge (lent composition-pieces))) :: f(X)=X^D
    //       (~(slag fop weights) num)
    //   ==
    let x_poly = new_fpoly(&[fpow_(deep_challenge, composition_pieces.len() as u64)]);

    let (pieces, _) = weighted_linear_combo(
        stack,
        &composition_pieces,
        composition_piece_openings,
        0,
        (&x_poly).into(),
        PolySlice(slag_ref(num, weights.0)),
    )?;

    /*println!(
        "PIECES @ pieces={} acc={}",
        vmug(stack, &pieces.0),
        vmug(stack, &acc.0)
    );*/

    // (fpadd acc pieces)
    let acc = fpadd(acc, (&pieces).into());

    //println!("ADDED acc={}", vmug(stack, &acc.0));

    //let res = IndirectAtom::from_raw_pointer(acc.0.as_ptr() as *const u64);
    //let res = IndirectAtom::new_raw_bytes(allocator, size, data)
    let (res, res_poly): (IndirectAtom, &mut [Felt]) =
        new_handle_mut_slice(stack, Some(acc.0.len()));
    res_poly.copy_from_slice(&acc.0);
    let res_cell = finalize_poly(stack, Some(acc.0.len()), res);
    Ok(res_cell)
    //Err(JetErr::Punt)
}

/*pub fn bpdiv(stack: &mut NockStack, inp: Noun) -> Result {
    let [a, b] = pull_args(inp)?;
    let al = bpoly_to_list(stack, a)?;
    let bl = bpoly_to_list(stack, b)?;
    println!("BPDIV {:?} {:?} | {:?} {:?}", DP(a), DP(b), DP(al), DP(bl));
    Err(JetErr::Punt)
}*/

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

    let Ok(trace_evals) = BPolySlice::try_from(trace_evals) else {
        return jet_err();
    };
    let chal_map = HoonMap::try_from(chal_map).ok();
    let Ok(dyns) = BPolySlice::try_from(dyns) else {
        return jet_err();
    };
    let com_map = HoonMapIter::try_from(com_map)
        .ok()
        .map(|v| {
            v.map(|v| {
                let [k, v] = v.uncell().unwrap();
                let v = BPolySlice::try_from(v).unwrap();
                (
                    k.as_atom().unwrap().as_u64().unwrap(),
                    PolyVec(v.0.to_vec()),
                )
            })
            .collect()
        })
        .unwrap_or_default();
    // println!("p={:?}", mug(stack, p).data());
    // println!("trace_evals={:?}", mug(stack, trace_evals).data());
    // println!("height={:?}", height);
    // println!("chal_map={:?}", mug(stack, chal_map).data());
    // println!("dyns={:?}", mug(stack, dyns).data());
    // println!("com_map={:?}", mug(stack, com_map).data());
    let acc = mp_substitute_mega_impl(
        stack,
        p,
        trace_evals,
        height.as_atom()?.as_u64()?,
        chal_map,
        dyns,
        &com_map,
    )?;

    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc.0);
    let ret = finalize_poly(stack, Some(acc.len()), ret);

    Ok(ret)
}

pub fn mp_substitute_mega_impl(
    stack: &mut NockStack,
    p: Noun,
    trace_evals: BPolySlice,
    height: u64,
    chal_map: Option<HoonMap>,
    dyns: BPolySlice,
    com_map: &BTreeMap<u64, BPolyVec>,
) -> core::result::Result<BPolyVec, JetErr> {
    // ^-  bpoly

    // %+  roll  ~(tap by p)
    // |=  [[k=bpoly v=belt] acc=_zero-bpoly]
    let acc = HoonMapIter::from(p).try_fold(PolyVec(vec![Belt(0)]), |acc, e| {
        let [k, v] = e.uncell()?;
        let Ok(k) = BPolySlice::try_from(k) else {
            return jet_err();
        };
        let v = Belt(v.as_atom()?.as_u64()?);

        // =/  [poly=bpoly len=@]  [trace-evals (mul 4 height)]
        let poly = trace_evals;
        let len = (height * 4) as usize;
        // println!("trace-evals: poly={:?}, len={:?}", mug(stack, poly), len);

        // =/  ones=bpoly  (init-bpoly (reap len 1))
        let ones = PolyVec(vec![Belt(1); len]);

        // ?:  =(v 0)  acc
        if v == Belt(0) {
            return Ok(acc);
        }

        // %+  bpadd  acc
        // %+  bpscal  v
        // %+  roll  (range len.k)
        // |=  [i=@ acc=_ones]
        // ^-  bpoly
        let mut rolled =
            k.0.iter()
                .copied()
                // =/  [typ=mega-typ:mp-to-mega idx=@ exp=@ud]
                //   (brek:mp-to-mega ter)
                .map(brek)
                .try_fold(ones, |mut acc, (typ, idx, exp)| {
                    // ?-  typ
                    Ok::<_, JetErr>(match typ {
                        // %var
                        MegaTyp::Var => {
                            // =/  var=bpoly  (~(swag bop poly) (mul idx len) len)
                            let a = poly.0.split_at(idx * len).1;
                            let var = PolySlice(&a[..core::cmp::min(a.len(), len)]);
                            // %+  roll  (range exp)
                            // |=  [i=@ power=_acc]
                            for _ in 0..exp {
                                // (bp-hadamard power var)
                                bp_hadamard_inplace(&mut acc.0, var.0);
                            }
                            acc
                        }
                        // %rnd
                        MegaTyp::Rnd => {
                            // =/  rnd  (~(got by chal-map) idx)
                            let rnd = chal_map
                                .and_then(|v| v.get(stack, D(idx as u64)))
                                .unwrap()
                                .1;
                            let rnd = rnd.as_atom()?.as_u64()?;
                            // (bpscal (bpow rnd exp) acc)
                            let powed = bpow(rnd, exp);
                            bpscal_inplace(Belt(powed), &mut acc.0);
                            acc
                        }
                        // %dyn
                        MegaTyp::Dyn => {
                            // =/  dyn  (~(snag bop dyns) idx)
                            let _dyn = dyns.0[idx];
                            // (bpscal (bpow dyn exp) acc)
                            let powed = bpow(_dyn.0, exp);
                            bpscal_inplace(Belt(powed), &mut acc.0);
                            acc
                        }
                        // %con
                        MegaTyp::Con => {
                            // acc
                            acc
                        }
                        // %com
                        MegaTyp::Com => {
                            // =/  com=bpoly  (~(got by com-map) idx)
                            let com = com_map.get(&(idx as u64)).unwrap();
                            // %+  roll  (range exp)
                            // |=  [i=@ power=_acc]
                            for _ in 0..exp {
                                // (bp-hadamard power com)
                                bp_hadamard_inplace(&mut acc.0, &com.0);
                            }
                            acc
                        }
                    })
                })?;

        // :: %+  bpscal  v
        bpscal_inplace(v, &mut rolled.0);
        // :: %+  bpadd  acc
        bpadd_in_place(&mut rolled.0, &acc.0);
        Ok(rolled)
    })?;

    Ok(acc)
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

// :: $mp-mega: multivariate polynomials in their final form
// ::
// ::    The multivariate polynomial is stored in a sparse map like in the multi-poly data type.
// ::    For each monomial term, there is a key and a value. The value is just the belt coefficient.
// ::    The key is a bpoly which packs in each element of the monomial. It looks like this:
// ::
// ::    [term term term ... term]=bpoly
// ::
// ::    where each term is one 64-bit direct atom. The format of a term is this:
// ::
// ::    3 bits - type of term
// ::    10 bits - index of term into list of variables / challenges / dynamics
// ::    30 bits - exponent as @ud
// ::
// ::    [TTIIIIIIIIIIEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE]
// ::
// ::    This only uses 43 bits which is plenty since the exponent can only be max 4 anyway.
// ::    So it safely fits inside a direct atom.
// ::
// ::    The type of term can be:
// ::      con - constant (so it's just the zero bpoly and the coefficient is the value)
// ::      var - variable. the index is the index of the variable.
// ::      rnd - random challenge from the verifier. the index is the index into the challenge list.
// ::      dyn - dynamic element so terminal. the index is the index into the dynamic list.
// ::
// ::    The reason for this is that the constraints are static and so we would like to build
// ::    them into an efficient data structure during a preprocess step and not every time we
// ::    generate a proof. The problem is that we don't know the challenges or the dynamics until
// ::    we are in the middle of generating a proof. So we store the index of the challenges and
// ::    dynamics in the data structure and read them out when we evaluate or substitute the polys.
// ::
// +$  mp-mega  (map bpoly belt)
// +$  mp-comp  [dep=(list mp-mega) com=(list mp-mega)]
// +$  mp-ultra
//   $%  [%mega mp-mega]
//       [%comp mp-comp]
//   ==
// ::  mp-ultra constraint along with corresponding degrees of the constraints inside
// +$  constraint-data  [cs=mp-ultra degs=(list @)]
// ::  all constraints for one table
// +$  constraints
//   $:  boundary=(list constraint-data)
//       row=(list constraint-data)
//       transition=(list constraint-data)
//       terminal=(list constraint-data)
//       extra=(list constraint-data)
//   ==
// +$  constraint-counts
//   $:  boundary=@
//       row=@
//       transition=@
//       terminal=@
//       extra=@
//   ==

fn compute_composition_poly(stack: &mut NockStack, sam: Noun) -> Result {
    // ~/  %compute-composition-poly
    // |=  $:  omicrons=bpoly
    //         heights=(list @)
    //         tworow-trace-polys=(list bpoly)
    //         constraint-map=(map @ constraints)
    //         constraint-counts=(map @ constraint-counts)
    //         composition-chals=(map @ bpoly)
    //         chal-map=(map @ belt)
    //         dyn-map=(map @ bpoly)
    //         is-extra=?
    //     ==
    // ^-  bpoly
    let [omicrons, heights, tworow_trace_polys, constraint_map, constraint_counts, composition_chals, chal_map, dyn_map, is_extra] =
        sam.uncell()?;

    let Ok(omicrons) = BPolySlice::try_from(omicrons) else {
        return jet_err();
    };

    let Ok(heights) = HoonList::try_from(heights).map(|v| {
        v.map(|v| v.as_atom().unwrap().as_u64().unwrap())
            .collect::<Vec<_>>()
    }) else {
        return jet_err();
    };

    let Ok(tworow_trace_polys) = HoonList::try_from(tworow_trace_polys).map(|v| {
        v.map(|v| BPolySlice::try_from(v).unwrap())
            .collect::<Vec<_>>()
    }) else {
        return jet_err();
    };

    let [constraint_map, constraint_counts, composition_chals, chal_map, dyn_map] = [
        constraint_map,
        constraint_counts,
        composition_chals,
        chal_map,
        dyn_map,
    ]
    .map(HoonMap::try_from)
    .map(|v| v.ok());

    let is_extra = is_extra.as_direct()?.data() == 0;

    // =/  max-height=@
    //   %-  bex  %-  xeb  %-  dec
    //   (roll heights max)
    let Some(&max_height) = heights.iter().max() else {
        return jet_err();
    };
    let max_height = 1 << xeb((max_height as usize) - 1);

    // =/  dp  (degree-processing heights constraint-map is-extra)
    let (fri_deg_bound, constraint_w_deg_map) =
        degree_processing(stack, &heights, constraint_map, is_extra)?;
    //let dp = HoonMap::try_from(dp).ok();

    // |^
    // =/  boundary-zerofier  (init-bpoly ~[(bneg 1) 1])          ::  f(X)=X-1
    let boundary_zerofier = [Belt(bneg(1)), Belt(1)];
    let boundary_zerofier = PolySlice(&boundary_zerofier);
    // ::
    // %+  roll  (range len.omicrons)
    // |=  [i=@ acc=_zero-bpoly]
    let mut acc = PolyVec(vec![Belt(0)]);
    for i in 0..omicrons.len() {
        // =/  height=@  (snag i heights)
        let height = heights[i];
        // =/  omicron  (~(snag bop omicrons) i)
        let omicron = omicrons.0[i];
        // =/  last-row  (init-bpoly ~[(bneg (binv omicron)) 1])      ::  f(X)=X-g^{-1}
        let last_row = [Belt(bneg(binv(omicron.0))), Belt(1)];
        let last_row = PolySlice(&last_row);
        // =/  chals  (~(got by composition-chals) i)
        let chals = composition_chals
            .and_then(|v| v.get(stack, D(i as _)))
            .ok_or_else(det_err)?
            .1;
        let chals2 = BPolySlice::try_from(chals)?;
        // =/  trace  (snag i tworow-trace-polys)
        let trace = tworow_trace_polys[i];
        // =/  constraints  (~(got by constraint-w-deg-map.dp) i)
        let constraints2 = constraint_w_deg_map.get(&(i as u64)).unwrap();
        // =/  counts  (~(got by constraint-counts) i)
        let counts = constraint_counts
            .and_then(|v| v.get(stack, D(i as _)))
            .ok_or_else(det_err)?
            .1;
        let counts: [_; 5] = counts
            .uncell()?
            .map(|v| v.as_atom().unwrap().as_u64().unwrap());
        // =/  dyns  (~(got by dyn-map) i)
        let dyns = dyn_map
            .and_then(|v| v.get(stack, D(i as _)))
            .ok_or_else(det_err)?
            .1;
        let dyns = BPolySlice::try_from(dyns)?;
        // ::
        // =/  row-zerofier                                           ::  f(X) = (X^N-1)
        //   (bpsub (bppow id-bpoly height) one-bpoly)
        let row_zerofier = bppow(&[Belt(0), Belt(1)], height as _);
        let row_zerofier = bpsub_(&row_zerofier, &[Belt(1)]);
        let row_zerofier = PolySlice(&row_zerofier);

        // ::  note: the transition zerofier = row-zerofier/last-row
        // ::  here, we are computing composition-constraints/transition-zerofier
        let transition_zerofier = bpdiv(row_zerofier.0, last_row.0);
        let transition_zerofier = PolySlice(&transition_zerofier);

        let dividends = [
            boundary_zerofier,
            row_zerofier,
            transition_zerofier,
            last_row,
            row_zerofier,
        ];

        let mut chals = chals2.0;
        for (o, ((constraints, count), dividend)) in
            constraints2.iter().zip(counts).zip(dividends).enumerate()
        {
            //   ?.  is-extra  zero-bpoly
            if o == dividends.len() - 1 && !is_extra {
                continue;
            }

            // NOTE: not in order here, and different iterations have diff parameters
            // (~(scag bop chals) (mul 2 boundary.counts))
            let (weights, next_chals) = chals.split_at(2 * (count as usize));
            chals = next_chals;
            // %-  process-composition-constraints
            // :*  boundary.constraints
            //     trace
            //     (~(scag bop chals) (mul 2 boundary.counts))
            //     dyns
            // ==
            let processed_constraints = process_composition_constraints(
                stack,
                constraints,
                trace,
                PolySlice(weights),
                dyns,
                fri_deg_bound,
                max_height,
                chal_map,
            )?;
            // %-  bpdiv
            // :_  boundary-zerofier
            let res = bpdiv(&processed_constraints.0, dividend.0);
            // ;:  bpadd
            //   acc
            acc.0
                .resize(core::cmp::max(acc.0.len(), res.len()), Belt(0));
            bpadd_in_place(&mut acc.0, &res);
        }
    }

    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc.0);
    let ret = finalize_poly(stack, Some(acc.len()), ret);

    Ok(ret)
}

fn process_composition_constraints(
    stack: &mut NockStack,
    constraints: &ProcessedDeg,
    trace: BPolySlice,
    weights: BPolySlice,
    dyns: BPolySlice,
    fri_deg_bound: u64,
    max_height: u64,
    chal_map: Option<HoonMap>,
) -> core::result::Result<BPolyVec, JetErr> {
    // |=  $:  constraints=(list [(list @) mp-ultra])
    //         trace=bpoly
    //         weights=bpoly
    //         dyns=bpoly
    //     ==
    // =-  (bpcan acc)
    // %+  roll  constraints
    // |=  [[degs=(list @) mp=mp-ultra] [idx=@ acc=_zero-bpoly]]
    // ::
    // ::  mp-substitute-ultra returns a list because the %comp
    // ::  constraint type can contain multiple mp-mega constraints.
    // ::
    let mut acc = PolyVec(vec![Belt(0)]);
    let mut idx = 0;
    for (degs, mp) in constraints.iter() {
        // =/  comps=(list bpoly)
        //   (mp-substitute-ultra mp trace max-height chal-map dyns)
        let comps = mp_substitute_ultra_impl(stack, *mp, trace, max_height, chal_map, dyns)?;
        // NOTE: zip-up expects equal lengths
        // %+  roll
        //   (zip-up degs comps)
        // |=  [[deg=@ comp=bpoly] [idx=_idx acc=_acc]]
        for (deg, comp) in degs.iter().zip(comps) {
            // :-  +(idx)
            // ::
            // ::  Each constraint corresponds to two weights: alpha and beta. The verifier
            // ::  samples 2*num_constraints random values and we assume that the alpha
            // ::  and beta weights for a given constraint are situated next to each other
            // ::  in the array.
            // ::
            // =/  alpha  (~(snag bop weights) (mul 2 idx))
            let alpha = weights.0[2 * idx];
            // =/  beta   (~(snag bop weights) (add 1 (mul 2 idx)))
            let beta = weights.0[1 + 2 * idx];
            // ::
            // ::  adjust degree up to fri-deg-bound.
            // ::  if fri-deg-bound is D-1 then we construct:
            // ::  p(x)*(α*X^{D-1-D_j} + β)
            // ::  which will make the polynomial exactly degree D-1 which is what we want.
            // =/  comp-coeff  (bp-ifft comp)
            let comp_coeff = bp_ifft(comp)?;
            // %+  bpadd  acc
            // %+  bpadd
            //   (bpscal beta comp-coeff)
            let mut beta_vec = comp_coeff.clone();
            bpscal_inplace(beta, &mut beta_vec.0);
            // %-  %~  weld  bop
            //     (init-bpoly (reap (sub fri-deg-bound.dp deg) 0))
            let mut alpha_vec = vec![Belt(0); (fri_deg_bound - *deg) as usize];
            alpha_vec.extend(comp_coeff.0.clone());
            // (bpscal alpha comp-coeff)
            bpscal_inplace(alpha, &mut alpha_vec);
            bpadd_in_place(&mut alpha_vec, &beta_vec.0);
            let acc_len = acc.len();
            acc.0
                .resize(core::cmp::max(acc_len, alpha_vec.len()), Belt(0));
            bpadd_in_place(&mut acc.0, &alpha_vec);
            idx += 1;
        }
    }

    Ok(bpcan(acc))
}

type ProcessedDeg = Vec<(Vec<u64>, Noun)>;

fn degree_processing(
    stack: &mut NockStack,
    heights: &[u64],
    constraint_map: Option<HoonMap>,
    is_extra: bool,
) -> core::result::Result<(u64, BTreeMap<u64, [ProcessedDeg; 5]>), JetErr> {
    // |=  [heights=(list @) constraint-map=(map @ constraints) is-extra=?]
    // ^-  [fri-deg-bound=@ constraint-w-deg-map=(map @ constraints-w-deg)]
    // =-  [(dec (bex (xeb (dec d)))) m]
    // %+  roll  (range (lent heights))
    // |=  [i=@ d=@ m=(map @ constraints-w-deg)]
    let mut d = 0u64;
    let mut m = BTreeMap::new();
    for (i, height) in heights.iter().copied().enumerate() {
        // =/  height=@  (snag i heights)
        // =/  constraints  (~(got by constraint-map) i)
        let (_, constraints) = constraint_map
            .and_then(|v| v.get(stack, D(i as u64)))
            .unwrap();
        let constraints: [_; 5] = constraints.uncell()?;
        let constraint_f = [
            // :: bnd
            // |=  deg=@
            // ?:  =(height 1)  0
            // (dec (mul deg (dec height)))
            |deg: u64, height: u64| {
                if height == 1 {
                    0
                } else {
                    deg * (height - 1) - 1
                }
            },
            // :: row
            // |=  deg=@
            // ?:  ?|(=(height 1) =(deg 1))  0
            // (sub (mul deg (dec height)) height)
            |deg: u64, height: u64| {
                if height == 1 || deg == 1 {
                    0
                } else {
                    deg * (height - 1) - height
                }
            },
            // :: trn
            // |=(@ (mul (dec +<) (dec height)))
            |deg: u64, height: u64| (deg - 1) * (height - 1),
            // :: trm
            // |=  deg=@
            // ?:  =(height 1)  0
            // (dec (mul deg (dec height)))
            |deg: u64, height: u64| {
                if height == 1 {
                    0
                } else {
                    deg * (height - 1) - 1
                }
            },
            // :: xta
            // |=  deg=@
            // ?:  ?|(=(height 1) =(deg 1))  0
            // (sub (mul deg (dec height)) height)
            |deg: u64, height: u64| {
                if height == 1 || deg == 1 {
                    0
                } else {
                    deg * (height - 1) - height
                }
            },
        ];
        // =-  :-  :(max d d.bnd d.row d.trn d.trm d.xta)
        //     (~(put by m) i [c.bnd c.row c.trn c.trm c.xta])
        // ::  attach composition degree to each mp & keep a running max of degrees
        // ::  divided by boundary, row, transition, terminal
        // NOTE: <X>=[c d] here
        // :*
        //   ^=  bnd=[c d]
        let mut res = [const { None }; 5];
        for (i, (constraints, func)) in constraints.into_iter().zip(constraint_f).enumerate() {
            let mut d = 0;
            let mut mapped_constraints = vec![];

            // NOTE: <X>.constraints here
            // %^  spin  boundary.constraints  0
            // NOTE: xta is last and has the following divergence
            // ?.  is-extra  [~ 0]
            if i != constraint_f.len() - 1 || is_extra {
                let constraints = HoonList::try_from(constraints).ok();

                for cd in constraints.into_iter().flatten() {
                    // |=  [cd=constraint-data d=@]
                    let [cs, degs] = cd.uncell()?;
                    let degs = HoonList::try_from(degs)?;

                    // =;  degrees=(list @)
                    //   :-  [degrees cs.cd]
                    //   (roll `(list @)`[d degrees] max)
                    // %+  turn  degs.cd
                    // |=  deg=@
                    // ?:  =(height 1)  0
                    // (dec (mul deg (dec height)))
                    let degrees = degs
                        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
                        .map(|deg| func(deg, height))
                        .collect::<Vec<_>>();

                    d = core::cmp::max(d, degrees.iter().copied().max().unwrap_or(d));
                    mapped_constraints.push((degrees, cs));
                }
            }
            res[i] = Some((mapped_constraints, d));
        }
        let res = res.map(Option::unwrap);
        // ==
        // NOTE: the =- p part
        d = core::cmp::max(d, res.iter().map(|(_, d)| *d).max().unwrap_or(d));
        //prinltn!("D_MAX {d}");
        m.insert(i as u64, res.map(|(a, _)| a));
    }

    Ok(((1 << xeb((d - 1) as usize)) - 1, m))
}
