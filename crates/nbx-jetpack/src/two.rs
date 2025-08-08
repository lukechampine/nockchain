use std::collections::BTreeMap;
use std::iter::{repeat, repeat_n};

use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::*;
use nockvm_macros::tas;
use crate::log::*;
use zkvm_jetpack::form::math::mary::mary_transpose;

use crate::one::snag_as_poly_mary;
use crate::three::mary_to_noun;

use super::one::{p_decompose_impl, peval_impl};
use super::substitute::{SubstituteEngine, SubstituteMulStage, SubstituteOp};
use super::utils::*;
use zkvm_jetpack::form::bpoly::{bp_coseword, bpscal_inplace};
use zkvm_jetpack::form::fext::{fadd_, fdiv_, finv_, fmul_, fneg_};
use zkvm_jetpack::form::mary::{Mary, MarySlice, MarySliceMut};
use zkvm_jetpack::form::math::poly::{p_ntt, *};
use zkvm_jetpack::form::mega::{brek, MegaTyp};
use zkvm_jetpack::form::poly::Poly;
use zkvm_jetpack::form::{
    binv, bneg, bpow, BPolySlice, BPolyVec, Belt, Element, ElementEx, FPolySlice, FPolySliceMut, FPolyVec, Felt, MPolyVec, PolySlice, PolyVec
};
use zkvm_jetpack::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_felt, new_handle_mut_mary, new_handle_mut_slice,
};
use zkvm_jetpack::hand::structs::{HoonList, HoonMapIter};
use zkvm_jetpack::jets::utils::jet_err;
use zkvm_jetpack::noun::noun_ext::NounExt;

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

pub fn bp_intercosate_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let [offset, order, values] = sam.uncell()?;
    let offset = Belt(offset.as_atom()?.as_u64()?);
    let order = order.as_atom()?.as_u64()? as u32;
    let values = BPolySlice::try_from(values)?;
    let r = bp_intercosate(offset, order, PolyVec(values.0.to_vec()));
    let (h, p) = new_handle_mut_slice(stack, Some(r.len()));
    p.copy_from_slice(&r.0);
    Ok(finalize_poly(stack, Some(r.len()), h))
}

pub fn bp_intercosate(offset: Belt, order: u32, values: BPolyVec) -> BPolyVec {
    // |=  [offset=belt order=@ values=bpoly]
    // ^-  bpoly
    // ~+
    // ::  order = |H| is a power of 2
    // ?>  =((dis order (dec order)) 0)
    // ::  number of values should match the number of points in the coset
    // ?>  =(len.values order)
    assert_eq!(values.len(), order as usize);
    // =/  ifft  (bp-ifft values)
    let mut ifft = p_ifft(values.0).unwrap();
    // (bp-shift (bp-ifft values) (binv offset))
    p_shift_inplace(&mut ifft, &Belt(binv(offset.0)));
    PolyVec(ifft)
}

pub fn interpolate_table_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let [table, domain_len] = sam.uncell()?;
    let Ok(table) = MarySlice::try_from(table) else { return jet_err() };
    let domain_len = domain_len.as_atom()?.as_u64()? as u32;
    let ma = interpolate_table(table, domain_len);
    Ok(mary_to_noun(stack, ma))
}

pub fn interpolate_table(table: MarySlice, domain_len: u32) -> Mary {
    // |=  [table=mary domain-len=@]
    // ^-  mary
    // =/  trace=mary  (transpose-bpolys table)
    let mut trace = Mary {
        dat: vec![0; table.dat.len()],
        len: table.step,
        step: table.len,
    };
    mary_transpose(table, 1, &mut trace.as_mut_slice());

    let mut out_trace = Mary {
        dat: Vec::with_capacity(table.dat.len()),
        len: trace.len,
        step: domain_len,
    };
    // %-  zing-bpolys
    // %+  turn  (range len.array.trace)
    // |=  col=@
    for col in 0..trace.len {
        // ^-  bpoly
        // =/  values-old=bpoly  (~(snag-as-bpoly ave trace) col)
        let values_old = snag_as_poly_mary::<Belt>(trace.as_slice(), col as usize);
        // =/  values  (~(zero-extend bop values-old) (sub domain-len len.values-old))
        let mut values = values_old.0.to_vec();
        values.resize(domain_len as usize, Belt(0));
        // ?>  =(len.values domain-len)
        // ?>  =((dis domain-len (dec domain-len)) 0)
        // (bp-intercosate 1 domain-len values)
        let res = bp_intercosate(Belt(1), domain_len, PolyVec(values));
        out_trace.dat.extend(res.0.into_iter().map(|v| v.0));
    }

    out_trace
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

pub fn bpeval_lift_sam(stack: &mut NockStack, sam: Noun) -> Result {
    // |:  [bp=`bpoly`one-bpoly x=`felt`(lift 1)]
    let [bp, x] = sam.uncell()?;
    let Ok(bp) = BPolySlice::try_from(bp) else {
        return jet_err();
    };
    let x = x.as_felt().copied().unwrap_or_else(|_| Felt::one());
    // ^-  felt
    let (r, h) = new_handle_mut_felt(stack);
    *h = bpeval_lift(bp, x);
    Ok(r.as_noun())
}

// ::  +bpeval-lift: evaluate a bpoly at a felt
pub fn bpeval_lift(bp: BPolySlice, x: Felt) -> Felt {
    // |:  [bp=`bpoly`one-bpoly x=`felt`(lift 1)]
    // ^-  felt
    // ~+
    // ?:  (bp-is-zero bp)  (lift 0)
    if bp.is_zero() {
        return Felt::zero();
    }
    // ?:  =(len.bp 1)  (lift (~(snag bop bp) 0))
    if bp.len() == 1 {
        return Felt::lift(bp.0[0]);
    }
    // =/  p  ~(to-poly bop bp)
    // =.  p  (flop p)
    // =/  res=@  (lift 0)
    // |-
    bp.0.iter().rev().fold(Felt::zero(), |res, p| {
        // ?~  p    !!
        // ?~  t.p
        //   (fadd (fmul res x) (lift i.p))
        // ::  based on p(x) = (...((a_n)x + a_{n-1})x + a_{n-2})x + ... )
        // $(res (fadd (fmul res x) (lift i.p)), p t.p)
        fadd_(&fmul_(&res, &x), &Felt::lift(*p))
    })
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

pub fn zeroextend_slice<'a, T: Copy>(mut a: PolyVec<T>, n: usize, zero: T) -> PolyVec<T> {
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

pub fn fp_is_zero(p: FPolySlice) -> bool {
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
    let pinned = pinv_mod_x_to(dq + 1, (&rg).into());
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

pub fn fp_ntt_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let [fp, root] = sam.uncell()?;

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

pub fn fp_fft_sam(stack: &mut NockStack, sam: Noun) -> Result {
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

pub fn fp_ifft_sam(stack: &mut NockStack, sam: Noun) -> Result {
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

//static mut CNT: usize = 0;

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
    let chal_map = HoonMapIter::try_from(chal_map)
        .ok()
        .into_iter()
        .flatten()
        .map(|v| {
            let [k, v] = v
                .uncell()
                .unwrap()
                .map(|v| v.as_atom().unwrap().as_u64().unwrap());
            (k, Belt(v))
        })
        .collect::<BTreeMap<_, _>>();

    let Ok(dyns) = BPolySlice::try_from(dyns) else {
        return jet_err();
    };

    //let mut engine = SubstituteEngine::new(height);
    //mp_substitute_ultra_impl::<Belt>(&mut engine, 0, p, trace_evals, &chal_map, dyns)?;
    //let (ret, poly_size) = engine.reduce_cpu();

    /*let mut engine = SubstituteEngine::new(height);
    let tevals = PolyVec(trace_evals.0.to_vec());
    let tevals: MPolyVec = tevals.into();
    mp_substitute_ultra_impl::<Melt>(&mut engine, 0, p, (&tevals).into(), &chal_map, dyns)?;
    let (ret, poly_size) = engine.clone().reduce_cpu();
    let (r2, poly_size) = engine.reduce();
    if ret != r2 {
        println!("RET MISMATCH R2 {:?} {:?}", &ret[0][..10], &r2[0][..10]);
        let cnt = unsafe {
            let ret = CNT;
            CNT += 1;
            ret
        };
        let path = format!("./jams/mp_substitute_ultra_mm_{cnt}");
        let _ = std::fs::create_dir_all(&path);
        let path = path.leak();
        return Err(JetErr::PuntJam(path));
    }

    let ret = ret.into_iter().map(|v| <PolyVec<Belt>>::from(PolyVec(v)).0).collect::<Vec<Vec<Belt>>>();*/

    let mut engine = SubstituteEngine::new(height);
    mp_substitute_ultra_impl::<Belt>(&mut engine, 0, p, trace_evals, &chal_map, dyns)?;
    let (ret, poly_size) = engine.reduce_cpu();

    let mut ret = ret
        .iter()
        .flat_map(|v| v.chunks(poly_size))
        .map(|v| {
            let (res, res_poly): (IndirectAtom, &mut [Belt]) =
                new_handle_mut_slice(stack, Some(v.len()));
            res_poly.copy_from_slice(v);
            let res_cell = finalize_poly(stack, Some(v.len()), res);
            res_cell
        })
        .collect::<Vec<_>>();
    ret.push(D(0));

    Ok(T(stack, &ret))
}

#[tracing::instrument(skip_all)]
pub fn mp_substitute_ultra_impl<'a, E: ElementEx>(
    engine: &mut SubstituteEngine<'a, E>,
    stage: usize,
    p: Noun,
    trace_evals: PolySlice<'a, E>,
    chal_map: &BTreeMap<u64, Belt>,
    dyns: BPolySlice,
) -> core::result::Result<usize, JetErr> {
    // ^-  (list bpoly)
    let [p_head, p_tail] = p.uncell()?;

    // ?-    -.p
    let ret = match p_head.as_direct()?.data() {
        // %mega
        tas!(b"mega") => {
            // :~  (mp-substitute-mega +.p trace-evals height chal-map dyns ~)
            // ==
            mp_substitute_mega_impl::<E, E>(
                engine,
                stage,
                p_tail,
                trace_evals,
                chal_map,
                dyns,
                &Default::default(),
            )?;

            1
        }
        // %comp
        tas!(b"comp") => {
            let [dep, com] = p_tail.uncell()?;
            let dep = HoonList::try_from(dep)?;
            let com = HoonList::try_from(com)?;
            // =;  com-map=(map @ bpoly)
            let mut com_map = BTreeMap::new();
            engine.ensure_stages(stage + 1);
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
                    mp_substitute_mega_impl::<E, E>(
                        engine,
                        stage + 1,
                        mp,
                        trace_evals,
                        chal_map,
                        dyns,
                        &Default::default(),
                    )?,
                );
            }

            let mut ret = 0;

            // %+  turn
            //   com.p
            for mp in com {
                // |=  mp=mp-mega
                // (mp-substitute-mega mp trace-evals height chal-map dyns com-map)
                mp_substitute_mega_impl::<E, E>(
                    engine, stage, mp, trace_evals, chal_map, dyns, &com_map,
                )?;
                ret += 1;
            }

            ret
        }
        // ==
        _ => jet_err()?,
    };

    Ok(ret)
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
pub fn fpmul_fast<'a>(fp: FPolyVec, fq: FPolyVec) -> FPolyVec {
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
    let twiddles = p_fft_twiddles::<Felt>(a.0.len()).unwrap();
    let mut a = PolyVec(p_ntt_twiddled(a.0, &twiddles));
    //   (fp-fft (~(zero-extend fop fq) (sub deg-prod deg-q)))
    let b = zeroextend_slice(fq, deg_prod, Felt::zero());
    let mut b = PolyVec(p_ntt_twiddled(b.0, &twiddles));
    // fmul
    a.0.truncate(b.0.len());
    b.0.truncate(a.0.len());
    a.0.iter_mut()
        .zip(b.0.into_iter())
        .for_each(|(a, b)| *a = fmul_(a, &b));
    fp_ifft(a).unwrap()
}

// ::  +fpmul: polynomial multiplication
#[tracing::instrument(skip_all)]
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
    fpmul_fast(fp, fq)
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

pub fn fdegree<T: Element + Copy + PartialEq>(p: PolySlice<T>) -> usize {
    // |=  p=poly
    // ^-  @
    // =/  cp=poly  (fcan p)
    // ?~  cp  0
    // (dec (lent cp))
    fcan(p).0.len().saturating_sub(1)
}

// ::  con-mon: split p(x)!=0 uniquely into c*f(x) where c is constant f monic
pub fn con_mon(mut fp: FPolyVec) -> (Felt, FPolyVec) {
    // |=  fp=fpoly
    // ^-  [felt fpoly]
    // ~+
    // =.  fp  ~(flop fop (fpcan fp))
    // ~|  "Cannot accept the zero polynomial!"
    // ?<  =(zero-fpoly fp)
    assert_ne!(fp.0, &[Felt::zero()]);
    // :-  ~(head fop fp)
    let head = fp.0[fp.0.len() - 1];
    // %~  flop  fop
    // (fpscal (finv ~(head fop fp)) fp)
    let head_inv = finv_(&head);
    let fp = fpscal(head_inv, fp);
    (head, fp)
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
    let [p, trace_evals, height, chal_map, dyns, com_map] = inp.uncell()?;

    let Ok(trace_evals) = BPolySlice::try_from(trace_evals) else {
        return jet_err();
    };
    let chal_map = HoonMapIter::try_from(chal_map)
        .ok()
        .into_iter()
        .flatten()
        .map(|v| {
            let [k, v] = v
                .uncell()
                .unwrap()
                .map(|v| v.as_atom().unwrap().as_u64().unwrap());
            (k, Belt(v))
        })
        .collect::<BTreeMap<_, _>>();
    let Ok(dyns) = BPolySlice::try_from(dyns) else {
        return jet_err();
    };

    let height = height.as_atom()?.as_u64()?;
    let mut engine = SubstituteEngine::new(height);
    engine.ensure_stages(1);
    let com_map = HoonMapIter::try_from(com_map)
        .ok()
        .map(|v| {
            v.map(|v| {
                let [k, v] = v.uncell().unwrap();
                let v = BPolySlice::try_from(v).unwrap();
                let v = PolyVec(v.0.to_vec());
                (
                    k.as_atom().unwrap().as_u64().unwrap(),
                    engine.push_iter(1, Some((&v).into()), PolySlice(&[])),
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
    mp_substitute_mega_impl::<Belt, Belt>(
        &mut engine, 0, p, trace_evals, &chal_map, dyns, &com_map,
    )?;

    let (mut acc, poly_len) = engine.reduce_cpu();
    assert_eq!(acc.len(), 1);
    assert_eq!(acc[0].len(), poly_len);
    let acc = PolyVec(acc.pop().unwrap());

    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc.0);
    let ret = finalize_poly(stack, Some(acc.len()), ret);

    Ok(ret)
}

#[inline(never)]
#[tracing::instrument(skip_all)]
pub fn mp_substitute_mega_impl<'a, E: ElementEx, P: Into<E> + Copy>(
    engine: &mut SubstituteEngine<'a, E>,
    stage: usize,
    p: Noun,
    trace_evals: PolySlice<'a, E>,
    chal_map: &BTreeMap<u64, Belt>,
    dyns: BPolySlice,
    com_map: &BTreeMap<u64, usize>,
    //com_map: &BTreeMap<u64, PolyVec<P>>,
) -> core::result::Result<usize, JetErr>
where
    PolyVec<P>: Into<PolyVec<E>>,
{
    // ^-  bpoly

    let len = engine.poly_len();
    let iter = engine.push_iter(stage, None, trace_evals);

    // %+  roll  ~(tap by p)
    // |=  [[k=bpoly v=belt] acc=_zero-bpoly]
    for e in HoonMapIter::from(p) {
        let [k, v] = e.uncell()?;
        let Ok(k) = BPolySlice::try_from(k) else {
            return jet_err();
        };
        let v = E::from_u64(v.as_atom()?.as_u64()?);

        // =/  [poly=bpoly len=@]  [trace-evals (mul 4 height)]

        // =/  ones=bpoly  (init-bpoly (reap len 1))

        // ?:  =(v 0)  acc
        if v.is_zero() {
            continue;
        }

        // %+  bpadd  acc
        // %+  bpscal  v
        // %+  roll  (range len.k)
        // |=  [i=@ acc=_ones]
        // ^-  bpoly
        let mut muls = SubstituteMulStage::new(v);

        for (typ, idx, exp) in
            k.0.iter()
                .copied()
                // =/  [typ=mega-typ:mp-to-mega idx=@ exp=@ud]
                //   (brek:mp-to-mega ter)
                .map(brek)
        {
            // ?-  typ
            match typ {
                // %var
                MegaTyp::Var => {
                    // =/  var=bpoly  (~(swag bop poly) (mul idx len) len)
                    // %+  roll  (range exp)
                    // |=  [i=@ power=_acc]
                    // (bp-hadamard power var)
                    muls.vars.push(SubstituteOp {
                        chunk: idx as _,
                        exp: exp as _,
                    });
                }
                // %rnd
                MegaTyp::Rnd => {
                    // =/  rnd  (~(got by chal-map) idx)
                    let rnd = chal_map.get(&(idx as u64)).unwrap();
                    // (bpscal (bpow rnd exp) acc)
                    let powed = bpow(rnd.0, exp);
                    muls.scal = muls.scal * E::from_u64(powed);
                }
                // %dyn
                MegaTyp::Dyn => {
                    // =/  dyn  (~(snag bop dyns) idx)
                    let _dyn = dyns.0[idx];
                    // (bpscal (bpow dyn exp) acc)
                    let powed = bpow(_dyn.0, exp);
                    muls.scal = muls.scal * E::from_u64(powed);
                }
                // %con
                MegaTyp::Con => {
                    // acc
                }
                // %com
                MegaTyp::Com => {
                    // =/  com=bpoly  (~(got by com-map) idx)
                    let com = *com_map.get(&(idx as u64)).unwrap();
                    // %+  roll  (range exp)
                    // |=  [i=@ power=_acc]
                    // (bp-hadamard power com)
                    muls.coms.push(SubstituteOp {
                        chunk: com as _,
                        exp: exp as _,
                    });
                }
            }
        }

        engine.push_mul(stage, iter, muls);
    }

    Ok(iter)
}

// ::  +pinv-mod-x-to: computes p^{-1} mod x^l
pub fn pinv_mod_x_to<'a>(l: usize, p: FPolySlice) -> FPolyVec {
    //jam_to(stack, p.0, "pmxt-p");
    //jam_to2(stack, D(l as _), "pmxt-l");
    // |=  [l=@ p=fpoly]
    // ^-  fpoly
    // (~(scag fop (hensel-lift-inverse p (xeb l))) l)
    let mut lifted = hensel_lift_inverse(p, xeb(l));
    lifted.0.truncate(l);
    //jam_to(stack, &lifted.0, "pmxt-r");
    lifted
}

// ::
// ::  +hensel-lift-inverse: if p_0 = 1, compute p^{-1} mod x^{2^l} (l = level parameter below)
// ::
// ::    Given a(x) such that p(x)a(x) = 1 mod x^{2^i}, then a*p = 1 + x^{2^i}s(x) (see s below).
// ::    Letting t(x) = -a(x)*s(x) mod x^{2^i}, then p's inverse modulo x^{2^{i+1}} is
// ::    a(x) + x^{2^i}t(x)
fn hensel_lift_inverse<'a>(p: FPolySlice, level: usize) -> FPolyVec {
    //jam_to(stack, p.0, "hli-p");
    //jam_to2(stack, D(level as _), "hli-l");
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
        let s = fpmul_fast(copy_slice(p), inv.clone());
        let s = PolyVec(slag_vec(bex_i, s.0));
        //println!("s {}", vmug(stack, s.0));
        // =/  t  (~(scag fop (fpmul (fpscal (lift (bneg 1)) inv) s)) bex-i)
        let t = fpscal(Felt::lift(Belt(bneg(1))), inv.clone());
        let mut t = fpmul_fast(t, s);
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
    //jam_to(stack, &inv.0, "hli-r");
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

// ::  +bp-ifft: Inverse DFT with FFT algorithm
pub fn bp_ifft<'a>(p: BPolyVec) -> core::result::Result<BPolyVec, JetErr> {
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

pub fn fat(stack: &mut NockStack, f: Felt) -> IndirectAtom {
    let (res, res_felt): (IndirectAtom, &mut Felt) = new_handle_mut_felt(stack);
    *res_felt = f;
    res
}

pub fn turn_coseword(stack: &mut NockStack, sam: Noun) -> Result {
    // ~/  %turn-coseword
    // |=  [polys=mary offset=belt order=@]
    let [polys, offset, order] = sam.uncell()?;
    let Ok(polys) = MarySlice::try_from(polys) else {
        return jet_err();
    };
    let offset = offset.as_belt()?;
    let order = order.as_direct()?.data() as u32;
    let root = Belt(order as _).ordered_root()?;

    // NOTE: bp-coseword returns a bpoly of length `order`
    let (ret_ma, h_ma) = new_handle_mut_mary(stack, order as _, polys.len as _);

    turn_coseword_impl(polys, offset, order, root, h_ma);

    // ^-  mary
    // %-  zing-bpolys
    Ok(finalize_mary(stack, order as _, polys.len as _, ret_ma))
}

pub fn turn_coseword_impl<T: ElementEx>(
    polys: MarySlice,
    offset: T,
    order: u32,
    root: T,
    out: MarySliceMut,
) {
    assert_eq!(out.step, order);
    assert_eq!(out.len, polys.len);
    assert_eq!(core::mem::size_of::<T>(), core::mem::size_of::<u64>());
    out.dat
        .chunks_mut(order as _)
        .zip(polys.dat.chunks_exact(polys.step as _))
        // %+  turn  (range len.array.polys)
        // |=  i=@
        // =/  bp=bpoly  (~(snag-as-bpoly ave polys) i)
        .map(|(a, bp)| {
            // SAFETY: Belt and u64 are equivalent
            unsafe {
                (
                    core::mem::transmute::<&mut [u64], &mut [T]>(a),
                    core::mem::transmute::<&[u64], &[T]>(bp),
                )
            }
        })
        .for_each(|(a, bp)| {
            // (bp-coseword bp offset order)
            a.copy_from_slice(&p_coseword(bp, &offset, order, &root));
        });
}

pub fn fp_decompose(stack: &mut NockStack, sam: Noun) -> Result {
    p_decompose_impl::<Felt>(stack, sam)
}

pub fn fpeval(stack: &mut NockStack, sam: Noun) -> Result {
    peval_impl::<Felt>(stack, sam)
}

pub fn lift_to_fpoly(stack: &mut NockStack, sam: Noun) -> Result {
    // |=  poly=(list belt)
    // ^-  fpoly
    // ?>  (levy poly based)
    // (init-fpoly (turn poly lift))
    let mut felts = vec![];

    for b in HoonList::try_from(sam).ok().into_iter().flatten() {
        felts.push(Felt::lift(Belt(b.as_atom()?.as_u64()?)));
    }

    let (ret, slc) = new_handle_mut_slice(stack, Some(felts.len()));
    slc.copy_from_slice(&felts);

    Ok(finalize_poly(stack, Some(felts.len()), ret))
}
