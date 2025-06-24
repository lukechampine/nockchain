use std::iter::once;

use crate::form::mary::{Mary, MarySlice};
use crate::form::math::poly::{p_decompose, peval};
use crate::form::{BPolySlice, Belt, Element, ElementEx, FPolySlice, Felt};
use crate::form::{BPolyVec, PolySlice};
use crate::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use crate::jets::utils::jet_err;
use crate::noun::noun_ext::NounExt;
use either::Either;
use ibig::Stack;
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

// ++  p  0xffff.ffff.0000.0001
// ++  r  0x1.0000.0000.0000.0000
// ++  r-mod-p  4.294.967.295
// ++  r2  0xffff.fffe.0000.0001
// ++  rp  0xffff.ffff.0000.0001.0000.0000.0000.0000
// ++  g  7
// ++  h  20.033.703.337

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

pub fn weld_step(context: &mut Context, subject: Noun) -> Result {
    let parent_core = slot(subject, 7)?;
    let ma = slot(parent_core, 6)?;
    let Ok(ma) = MarySlice::try_from(ma) else {
        return jet_err();
    };

    // ~/  %weld-step
    // |=  na=mary
    let na = slot(subject, 6)?;
    let Ok(na) = MarySlice::try_from(na) else {
        return jet_err();
    };

    // ^-  mary
    // ?>  =(len.array.ma len.array.na)
    assert_eq!(ma.len, na.len);

    // %+  roll  (range len.array.na)
    // =/  mu=mary
    //   :+  (add step.ma step.na)
    //     len.array.ma
    //   (lsh [6 (mul (add step.ma step.na) len.array.ma)] 1)
    let mu_step = ma.step + na.step;
    let (ret, mu) = new_handle_mut_mary(&mut context.stack, mu_step as usize, ma.len as usize);
    // |=  [i=@ mu=_mu]
    for (mu, (ma, na)) in mu.dat.chunks_exact_mut(mu_step as _).zip(
        ma.dat
            .chunks_exact(ma.step as _)
            .zip(na.dat.chunks_exact(na.step as _)),
    ) {
        // =;  weld-dat
        //   (~(stow ave mu) i weld-dat)
        // =/  r1  (snag i)
        // =?  r1  !=(step.ma 1)
        //   (sub r1 (lsh [6 step.ma] 1))
        // =/  r2  (~(snag ave na) i)
        // =?  r2  !=(step.na 1)
        //   (add (lsh [6 step.na] 1) r2)
        // (add (lsh [6 step.ma] r2) r1)
        let (m, n) = mu.split_at_mut(ma.len());
        m.copy_from_slice(ma);
        n.copy_from_slice(na);
    }

    Ok(finalize_mary(
        &mut context.stack,
        mu_step as usize,
        ma.len as usize,
        ret,
    ))
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

pub fn p_decompose_impl<T: ElementEx>(stack: &mut NockStack, sam: Noun) -> Result {
    let [p, d] = sam.uncell()?;
    let Ok(p) = PolySlice::try_from(p) else {
        return jet_err();
    };
    let d = d.as_atom()?.as_u64()? as usize;

    let r = p_decompose::<T>(p, d);
    let r = r
        .into_iter()
        .map(|v| {
            let (elem, p) = new_handle_mut_slice(stack, Some(v.len()));
            p.copy_from_slice(&v);
            finalize_poly(stack, Some(v.len()), elem)
        })
        .chain(once(D(0)))
        .collect::<Vec<_>>();

    Ok(T(stack, &r))
}

pub fn bp_decompose(stack: &mut NockStack, sam: Noun) -> Result {
    p_decompose_impl::<Belt>(stack, sam)
}

pub fn peval_impl<T: ElementEx>(stack: &mut NockStack, sam: Noun) -> Result {
    let [p, d] = sam.uncell()?;
    let Ok(p) = PolySlice::try_from(p) else {
        return jet_err();
    };
    let Ok(d) = T::try_from(d) else {
        return jet_err();
    };

    let r = peval::<T>(p, d);

    Ok(r.as_noun(stack))
}

pub fn bpeval(stack: &mut NockStack, sam: Noun) -> Result {
    peval_impl::<Belt>(stack, sam)
}
