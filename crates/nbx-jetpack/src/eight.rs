use crate::engine::Engine;
use std::collections::BTreeMap;
use std::rc::Rc;

use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, IndirectAtom, Noun, D, T};
use crate::deep::DeepEngine;
use crate::log::*;
use zkvm_jetpack::form::math::mary::mary_transpose;

use crate::codewords::CodewordEngine;
use crate::seven::height_mary;
use crate::three::{build_merk_heap_impl, mary_to_noun};

use super::one::*;
use super::substitute::SubstituteEngine;
use super::two::*;
use super::utils::*;
use zkvm_jetpack::form::fext::{fmul_, fpow_};
use zkvm_jetpack::form::mary::{Mary, MarySlice, MarySliceMut};
use zkvm_jetpack::form::math::poly::*;
use zkvm_jetpack::form::poly::Poly;
use zkvm_jetpack::form::{
    binv, bneg, BPolySlice, BPolyVec, Belt, Element, ElementEx, FPolySlice, FPolyVec, Felt, Melt,
    PolySlice, PolyVec,
};
use zkvm_jetpack::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use zkvm_jetpack::hand::structs::{HoonList, HoonMap, HoonMapIter};
use zkvm_jetpack::jets::utils::{det_err, jet_err};
use zkvm_jetpack::noun::noun_ext::NounExt;

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
        inp.uncell()?;

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

    let mut engine = DeepEngine::new(weights);

    //let mut acc = zero_fpoly();
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
                let bp = snag_as_poly_mary(p, i as usize);
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
            let new_num = engine.weighted_linear_combo(
                &lis,
                trace_openings,
                num,
                (&fpc_point).into(),
                num,
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
            let new_num = engine.weighted_linear_combo(
                &lis,
                trace_openings,
                new_num,
                (&point_omi_dc).into(),
                new_num,
            )?;
            //println!("SECOND-ROW {}", vmug(stack, &second_row.0));

            // :_  num
            num = new_num;
            // :(fpadd acc first-row second-row)
            //acc = fpadd(acc, (&first_row).into());
            //acc = fpadd(acc, (&second_row).into());
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

    engine.weighted_linear_combo(
        &composition_pieces,
        composition_piece_openings,
        num,
        (&x_poly).into(),
        0,
    )?;

    /*println!(
        "PIECES @ pieces={} acc={}",
        vmug(stack, &pieces.0),
        vmug(stack, &acc.0)
    );*/

    // (fpadd acc pieces)
    let acc = engine.reduce();

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

#[tracing::instrument(skip_all)]
pub fn compute_composition_poly(stack: &mut NockStack, sam: Noun) -> Result {
    // NOTE: in theory, using `Melt` should be faster than `Belt`, due to efficient multiplication,
    // however, for some reason LTO-d x86_64-v4 binary is faster with `Belt`. So here, we switch
    // against them.

    //#[cfg(target_feature = "avx2")]
    //type Elem = Belt;
    //#[cfg(not(target_feature = "avx2"))]
    type Elem = Melt;

    // ~/  %compute-composition-poly
    // |=  $:  omicrons=bpoly
    //         heights=(list @)
    //         tworow-trace-polys=(list bpoly)
    //         constraint-map=(map @ constraints)
    //         constraint-counts=(map @ constraint-counts)
    //         weights-map=(map @ bpoly)
    //         challenges=bpoly
    //         dyn-list=(list bpoly)
    //         is-extra=?
    //     ==
    // ^-  bpoly
    let [omicrons, heights, tworow_trace_polys, constraint_map, constraint_counts, weights_map, challenges, dyn_list, is_extra] =
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

    let Ok(challenges) = BPolySlice::try_from(challenges) else {
        return jet_err();
    };

    let Ok(dyn_list) = HoonList::try_from(dyn_list).map(|v| {
        v.map(|v| BPolySlice::try_from(v).unwrap())
            .collect::<Vec<_>>()
    }) else {
        return jet_err();
    };

    let [constraint_map, constraint_counts, weights_map] = [
        constraint_map,
        constraint_counts,
        weights_map,
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
    let boundary_zerofier = [Elem::from_u64(bneg(1)), Elem::one()];
    let boundary_zerofier = PolySlice(&boundary_zerofier);

    // Substitution moved out from process_degree_constraints to have everything done in one go.
    let mut engine = SubstituteEngine::new(max_height);
    let mut comp_cnts = vec![];
    let tworow_trace_polys = tworow_trace_polys
        .iter()
        .map(|v| PolyVec(v.0.to_vec()))
        .map(<PolyVec<Elem>>::from)
        .collect::<Vec<_>>();
    for i in 0..omicrons.len() {
        // =/  trace  (snag i tworow-trace-polys)
        let trace = &tworow_trace_polys[i];
        let trace: PolySlice<Elem> = trace.into();
        // =/  constraints  (~(got by constraint-w-deg-map.dp) i)
        let constraints2 = constraint_w_deg_map.get(&(i as u64)).unwrap();
        // =/  dyns  (snag i dyn-list)
        let dyns = dyn_list[i];

        for constraints in constraints2 {
            for (_, mp) in constraints.iter() {
                // =/  comps=(list bpoly)
                //   (mp-substitute-ultra mp trace max-height chal-map dyns)
                comp_cnts.push(mp_substitute_ultra_impl(
                        &mut engine,
                        0,
                        *mp,
                        trace,
                        challenges,
                        dyns,
                )?);
            }
        }
    }

    let (all_comps, poly_len) = engine.reduce();
    let mut all_comps = all_comps.iter().flat_map(|m| m.chunks(poly_len));
    let mut comp_cnts = comp_cnts.into_iter();

    // ::
    // %+  roll  (range len.omicrons)
    // |=  [i=@ acc=_zero-bpoly]
    let mut acc = PolyVec(vec![Elem::zero()]);
    for i in 0..omicrons.len() {
        // =/  height=@  (snag i heights)
        let height = heights[i];
        // =/  omicron  (~(snag bop omicrons) i)
        let omicron = omicrons.0[i];
        // =/  last-row  (init-bpoly ~[(bneg (binv omicron)) 1])      ::  f(X)=X-g^{-1}
        let last_row = [Elem::from_u64(bneg(binv(omicron.0))), Elem::one()];
        let last_row = PolySlice(&last_row);
        // =/  weights  (~(got by weights-map) i)
        let weights = weights_map
            .and_then(|v| v.get(stack, D(i as _)))
            .ok_or_else(det_err)?;
        let weights2 = BPolySlice::try_from(weights)?;
        // =/  counts  (~(got by constraint-counts) i)
        let counts = constraint_counts
            .and_then(|v| v.get(stack, D(i as _)))
            .ok_or_else(det_err)?;
        let counts: [_; 5] = counts
            .uncell()?
            .map(|v| v.as_atom().unwrap().as_u64().unwrap());
        // =/  constraints  (~(got by constraint-w-deg-map.dp) i)
        let constraints2 = constraint_w_deg_map.get(&(i as u64)).unwrap();
        // ::
        // =/  row-zerofier                                           ::  f(X) = (X^N-1)
        //   (bpsub (bppow id-bpoly height) one-bpoly)
        let row_zerofier = ppow(&[Elem::zero(), Elem::one()], height as _);
        let row_zerofier = psub_(&row_zerofier, &[Elem::one()]);
        let row_zerofier = PolySlice(&row_zerofier);

        // ::  note: the transition zerofier = row-zerofier/last-row
        // ::  here, we are computing composition-constraints/transition-zerofier
        let transition_zerofier = pdiv(row_zerofier.0, last_row.0);
        let transition_zerofier = PolySlice(&transition_zerofier);

        let dividends =
            [boundary_zerofier, row_zerofier, transition_zerofier, last_row, row_zerofier];

        let mut weights = weights2.0;
        for (o, ((constraints, count), dividend)) in
            constraints2.iter().zip(counts.into_iter()).zip(dividends).enumerate()
        {
            //   ?.  is-extra  zero-bpoly
            if o == dividends.len() - 1 && !is_extra {
                continue;
            }

            // NOTE: not in order here, and different iterations have diff parameters
            // (~(scag bop weights) (mul 2 boundary.counts))
            let (cur_weights, next_weights) = weights.split_at(2 * (count as usize));
            weights = next_weights;
            // %-  process-composition-constraints
            // :*  boundary.constraints
            //     trace
            //     (~(scag bop weights) (mul 2 boundary.counts))
            //     dyns
            // ==
            let processed_constraints = process_composition_constraints(
                &mut all_comps,
                &mut comp_cnts,
                constraints,
                PolySlice(cur_weights),
                fri_deg_bound,
            )?;
            // %-  bpdiv
            // :_  boundary-zerofier
            //#[cfg(not(target_feature = "avx2"))]
            let dividend: PolyVec<Elem> = PolyVec(dividend.0.to_vec()).into();
            let res = pdiv(&processed_constraints.0, &dividend.0);
            // ;:  bpadd
            //   acc
            acc.0
                .resize(core::cmp::max(acc.0.len(), res.len()), Elem::zero());
            padd_in_place(&mut acc.0, &res);
        }
    }

    let acc: BPolyVec = acc.into();

    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc.0);
    let ret = finalize_poly(stack, Some(acc.len()), ret);

    Ok(ret)
}

#[tracing::instrument(skip_all)]
fn process_composition_constraints<'a>(
    mut all_comps: impl Iterator<Item = &'a [Melt]>,
    comp_cnts: impl Iterator<Item = usize>,
    constraints: &ProcessedDeg,
    weights: BPolySlice,
    fri_deg_bound: u64,
) -> core::result::Result<PolyVec<Melt>, JetErr>
{
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
    let mut acc = PolyVec(vec![Melt::zero()]);
    let mut idx = 0;

    for ((degs, _), comps) in constraints.iter().zip(comp_cnts) {
        let comps = (&mut all_comps).take(comps);

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
            let comp_coeff = p_ifft(comp.to_vec())?;
            // %+  bpadd  acc
            // %+  bpadd
            //   (bpscal beta comp-coeff)
            let mut beta_vec = comp_coeff.clone();
            pscal_inplace(beta, &mut beta_vec);
            // %-  %~  weld  bop
            //     (init-bpoly (reap (sub fri-deg-bound.dp deg) 0))
            let mut alpha_vec = vec![Melt::zero(); (fri_deg_bound - *deg) as usize];
            alpha_vec.extend(comp_coeff.clone());
            // (bpscal alpha comp-coeff)
            pscal_inplace(alpha, &mut alpha_vec);
            padd_in_place(&mut alpha_vec, &beta_vec);
            let acc_len = acc.len();
            acc.0
                .resize(core::cmp::max(acc_len, alpha_vec.len()), Melt::zero());
            padd_in_place(&mut acc.0, &alpha_vec);
            idx += 1;
        }
    }

    Ok(PolyVec(pcan(acc.0)))
}

type ProcessedDeg = Vec<(Vec<u64>, Noun)>;

#[tracing::instrument(skip_all)]
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
        let constraints = constraint_map
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

pub fn precompute_ntts(stack: &mut NockStack, inp: Noun) -> Result {
    // |=  [polys=mary height=@ ntt-len=@]
    let [polys, height, ntt_len] = inp.uncell()?;
    let polys = MarySlice::try_from(polys).or_else(|_| jet_err())?;
    let height = height.as_direct()?.data() as usize;
    let ntt_len = ntt_len.as_direct()?.data() as usize;

    // ^-  bpoly
    // %-  need
    // =/  new-len  (mul height ntt-len)
    let new_len = height * ntt_len;
    let twiddles = p_fft_twiddles::<Belt>(new_len)?;

    // %+  roll  (range len.array.polys)
    let acc = (0..polys.len).try_fold(PolyVec(vec![]), |mut acc, i| {
        // |=  [i=@ acc=(unit bpoly)]
        // =/  p=bpoly  (~(snag-as-bpoly ave polys) i)
        let p = snag_as_poly_mary(polys, i as usize);
        let mut p = PolyVec(p.0.to_vec());

        // =/  fft=bpoly
        //   (bp-fft (~(zero-extend bop p) (sub new-len len.p)))
        p.0.resize(new_len, Belt(0));
        let fft = p_ntt_twiddled(p.0, &twiddles);

        // ?~  acc  (some fft)
        // (some (~(weld bop u.acc) fft))
        acc.0.extend_from_slice(&fft);
        Ok::<_, JetErr>(acc)
    })?;

    assert!(!acc.0.is_empty());

    let (res_atom, res_slice): (IndirectAtom, &mut [Belt]) =
        new_handle_mut_slice(stack, Some(acc.len()));

    res_slice.copy_from_slice(&acc.0);

    Ok(finalize_poly(stack, Some(acc.len()), res_atom))
}

#[tracing::instrument(skip_all)]
pub fn compute_table_polys(tables: &[MarySlice]) -> Vec<Mary> {
    // |=  tables=(list mary)
    // ^-  (list mary)
    // %+  turn  tables
    // |=  p=mary
    // =/  height  (height-mary:tlib p)
    // ?:  =(height 0)
    //   ~|("compute-table-polys: height 0 table detected" !!)
    // (interpolate-table p height)
    tables.iter().map(|p| interpolate_table(*p, height_mary(*p))).collect()
}

pub fn compute_codeword_commitments_sam(stack: &mut NockStack, sam: Noun) -> Result {
    // |=  $:  table-marys=(list mary)
    //         fri-domain-len=@
    //         total-cols=@
    //     ==
    let [table_marys, fri_domain_len, total_cols] = sam.uncell()?;
    let mut table_marys_vec = vec![];
    for m in HoonList::try_from(table_marys).ok().into_iter().flatten() {
        let Ok(ma) = MarySlice::try_from(m) else {
            return jet_err();
        };
        table_marys_vec.push(ma);
    }
    let fri_domain_len = fri_domain_len.as_atom()?.as_u64()? as u32;
    let total_cols = total_cols.as_atom()?.as_u64()?;
    // ^-  codeword-commitments
    // ::
    // ::  convert the ext columns to marys
    // ::
    // ::  think of each mary as a list of the table's columns, interpolated to polynomials
    // =/  table-polys=(list mary)
    //   (compute-table-polys table-marys)
    let table_polys_vec = compute_table_polys(&table_marys_vec);
    let table_polys = table_polys_vec.iter().map(MarySlice::from).collect::<Vec<_>>();
    let engine = CodewordEngine::new(table_polys, fri_domain_len, total_cols);
    let (codeword_array, height, mh) = engine.reduce();
    // [table-polys codeword-array merk-heap]
    let table_polys = table_polys_vec.into_iter().map(|v| mary_to_noun(stack, v)).chain([D(0)]).collect::<Vec<_>>();
    let table_polys = T(stack, &table_polys);
    let codeword_array = mary_to_noun(stack, codeword_array);
    let height = Atom::new(stack, height as _).as_noun();
    let mh = mh.to_noun(stack);
    let merk_heap = T(stack, &[height, mh]);
    Ok(T(stack, &[table_polys, codeword_array, merk_heap]))
}

pub fn compute_lde_sam(stack: &mut NockStack, sam: Noun) -> Result {
    // ~/  %compute-lde
    // |=  $:  table-polys=(list mary)
    //         fri-domain-len=@
    //         num-cols=@
    //     ==
    let [table_polys, fri_domain_len, num_cols] = sam.uncell()?;
    let mut table_polys_vec = vec![];
    for poly in HoonList::try_from(table_polys).ok().into_iter().flatten() {
        let Ok(poly) = MarySlice::try_from(poly) else {
            return jet_err();
        };
        table_polys_vec.push(poly);
    }
    let fri_domain_len = fri_domain_len.as_atom()?.as_u64()?;
    let num_cols = num_cols.as_atom()?.as_u64()?;
    let (h, mut ma) = new_handle_mut_mary(stack, fri_domain_len as _, num_cols as _);
    compute_lde::<Belt>(
        &table_polys_vec,
        fri_domain_len as _,
        num_cols,
        ma.as_mut_slice(),
    );
    // ^-  mary
    Ok(finalize_mary(stack, ma.step as _, ma.len as _, h))
}

#[tracing::instrument(skip_all)]
pub fn compute_lde<T: ElementEx>(
    table_polys: &[MarySlice],
    fri_domain_len: u32,
    num_cols: u64,
    out: MarySliceMut,
) where Belt: Into<T> {
    assert_eq!(out.step, fri_domain_len as u32);
    assert_eq!(out.len, num_cols as u32);
    // =/  fps=(list mary)
    //   %+  turn  table-polys
    //   |=  t=mary
    //   (turn-coseword t g fri-domain-len)
    // =/  res=mary
    //   :+  step=fri-domain-len
    //     len=num-cols
    //   dat=(lsh [6 (mul fri-domain-len num-cols)] 1)
    // =;  [@ ret=mary]
    //   ret
    // %+  roll
    //   fps
    // |=  [curr=mary [idx=@ res=_res]]
    // ?>  =(step.curr fri-domain-len)
    // =/  chunk  (mul step.curr len.array.curr)
    // :-  (add idx chunk)
    // res(dat.array (sew 6 [idx chunk dat.array.curr] dat.array.res))
    let fri_domain_root: T = Belt(fri_domain_len as _).ordered_root().unwrap().into();
    let mut out = out.dat;
    for ma in table_polys {
        let (cout, nout) = out.split_at_mut(fri_domain_len as usize * ma.len as usize);
        let ma_out = MarySliceMut {
            step: fri_domain_len as _,
            len: ma.len,
            dat: cout,
        };
        out = nout;
        turn_coseword_impl(*ma, G.into(), fri_domain_len, fri_domain_root, ma_out);
    }
}
