use crate::form::mary::MarySlice;
use crate::form::{BPolySlice, Belt, Element, FPolySlice, Felt};
use crate::form::{BPolyVec, PolySlice};
use crate::noun::noun_ext::NounExt;
use either::Either;
use nockvm::interpreter::Context;
use nockvm::jets::bits::util as bits;
use nockvm::jets::list::util as list;
use nockvm::jets::math::util as math;
use nockvm::jets::util::slot;
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, Cell, IndirectAtom, Noun, D, T};

use crate::jets::bp_jets::init_bpoly;

use super::utils::*;

pub fn mont_reduction(x: u128) -> u64 {
    // |=  x=melt
    // ^-  belt
    // ?>  (lth x rp)
    // assert!(x < RP);

    // ++  p  0xffff.ffff.0000.0001
    let p: u64 = 0xffffffff00000001;
    // ++  r  0x1.0000.0000.0000.0000
    // ++  r-mod-p  4.294.967.295
    // ++  r2  0xffff.fffe.0000.0001
    // ++  rp  0xffff.ffff.0000.0001.0000.0000.0000.0000
    // ++  g  7
    // ++  h  20.033.703.337

    // =/  x1  (cut 5 [1 1] x)
    let x1 = x as u64;

    // =/  x2  (rsh 6 x)
    let x2 = (x >> 64) as u64;

    // NOTE: the rest is different. see: https://docs.rs/twenty-first/latest/src/twenty_first/math/b_field_element.rs.html#340-353
    // =/  c
    //   =/  x0  (end 5 x)
    //   (lsh 5 (add x0 x1))
    // =/  f   (rsh 6 c)
    // =/  d   (sub c (add x1 (mul f p)))
    // ?:  (gte x2 d)
    //   (sub x2 d)
    // (sub (add x2 p) d)

    let (a, e) = x1.overflowing_add(x1 << 32);
    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = x2.overflowing_sub(b);

    r.wrapping_sub((1 + !p) * c as u64)
}

// ::  +montiply: computes a*b = (abr^{-1} mod p); note mul, not fmul: avoids mod p reduction!
pub fn montiply(a: u64, b: u64) -> u64 {
    // |:  [a=`melt`r-mod-p b=`melt`r-mod-p]
    // ^-  belt
    // ~+
    // ?>  ?&((based a) (based b))
    // FIXME: verify based
    mont_reduction((a as u128) * (b as u128))
}

// ::  +montify: transform to Montgomery space, i.e. compute x•r = xr mod p
pub fn montify(x: u64) -> u64 {
    // ++  r2  0xffff.fffe.0000.0001
    let r2: u64 = 0xfffffffe00000001;

    // |=  x=belt
    // ^-  melt
    // ~+
    // (montiply x r2)
    montiply(x, r2)
}

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

pub fn snag_as_poly_mary<'a, T: Element>(ma: MarySlice<'a>, i: usize) -> PolySlice<'a, T> {
    // ~/  %snag-as-fpoly
    // |=  i=@
    // ^-  fpoly
    // [(div step.ma 3) (snag i)]
    let dat = ma.dat.split_at(i * (ma.step as usize)).1;
    let dat = dat.split_at(ma.step as usize).0;
    let len = dat.len() / T::len();
    // SAFETY: The data slice always contains len number of poly elems inside
    PolySlice(unsafe { core::slice::from_raw_parts(dat.as_ptr() as *const T, len) })
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

pub fn do_init_mary(stack: &mut NockStack, inp: Noun) -> Result {
    // ~/  %do-init-mary
    // |=  [step=@ poly=(list elt)]
    let [step, poly] = inp.uncell()?;
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
        let poly_len = list::lent(poly)?;
        // =/  high-bit  (lsh [0 (mul (bex 6) (mul step (lent poly)))] 1)
        let bstep = (1 << 6) * step * poly_len;
        let high_bit = bits::lsh(stack, 0, bstep, D(1).as_atom()?)?.as_atom()?;
        // (add (rep [6 step] poly) high-bit)
        let repped = bits::rep(stack, 6, step, poly)?;
        let added = math::add(stack, repped, high_bit);

        Ok(T(stack, &[D(step as _), D(poly_len as _), added.as_noun()]))
    }
}

pub fn zero_extend(context: &mut Context, subject: Noun) -> Result {
    let parent_core = slot(subject, 7)?;
    let ma = slot(parent_core, 6)?;

    // |=  n=@
    // ^-  mary
    let n = slot(subject, 6)?.as_direct()?.data();

    let [step, len, dat] = ma.uncell()?;
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

pub fn bpcan(mut p: BPolyVec) -> BPolyVec {
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
