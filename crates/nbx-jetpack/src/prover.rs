use core::panic;

use nbx_tip5::base::{binv, bneg};
use nbx_tip5::melt::Melt;
use nockchain_math::belt::{bpow, Belt};
use nockchain_math::bpoly::bp_coseword;
use nockchain_math::felt::{fpow, Felt};
use nockchain_math::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use nockchain_math::mary::{mary_weld, MarySlice};
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::poly::{BPolySlice, *};
use nockchain_math::poly_ext::*;
use nockchain_math::structs::{HoonList, HoonMap, HoonMapIter};
use nockvm::jets::util::{slot, BAIL_FAIL};
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::{IndirectAtom, Noun, D, T};
use noun_serde::NounEncode;
use zkvm_jetpack::form::poly::Poly;
use zkvm_jetpack::jets::bp_jets::init_bpoly_bridge;
use zkvm_jetpack::jets::fp_jets::init_fpoly_bridge;
use zkvm_jetpack::jets::mary_jets::{
    snag_as_bpoly, snag_as_digest, snag_as_digest_jet, snag_one, transpose_bpolys,
};

use super::substitute::SubstituteEngine;
use super::two::*;
use crate::eight::{degree_processing, process_composition_constraints};
use crate::engine::Engine;
use crate::four::{absorb_proof_objects_impl, Proof, ProofData};
use crate::one::{do_init_mary, weld_marys_step, G};
use crate::seven::height_mary;
use crate::snag_as_poly_mary;
use crate::utils::xeb;

pub fn table_heights(stack: &mut NockStack, tables: Noun) -> Result {
    let tables = HoonList::try_from(tables)?;

    let mut heights = Vec::with_capacity(tables.count());
    for t in tables {
        // extract mary len without decoding everything
        let mary = t.as_cell()?.head().as_cell()?.tail();
        let mary_len = slot(mary, 6)?.as_atom()?.as_u64()?;
        let height = if mary_len == 0 {
            0
        } else {
            mary_len.next_power_of_two()
        };
        heights.push(D(height));
    }
    Ok(heights.to_noun(stack))
}

fn collect_marys(
    stack: &mut NockStack,
    tables: Noun,
    table_mary_axis: u64,
    width_axis: u64,
) -> Result {
    let tables = HoonList::try_from(tables)?;

    let mut marys = Vec::with_capacity(tables.count());
    let mut width = 0u64;
    for t in tables {
        let table_mary = slot(t, table_mary_axis)?;
        let header = slot(table_mary, 2)?;
        width += slot(header, width_axis)?.as_atom()?.as_u64()?;
        marys.push(slot(table_mary, 3)?);
    }

    let marys = marys.to_noun(stack);
    Ok(T(stack, &[marys, D(width)]))
}

pub fn bas_marys(stack: &mut NockStack, tables: Noun) -> Result {
    collect_marys(stack, tables, 2, 14)
}

pub fn ext_mary(stack: &mut NockStack, tables: Noun) -> Result {
    collect_marys(stack, tables, 1, 30)
}

pub fn mega_ext_mary(stack: &mut NockStack, tables: Noun) -> Result {
    collect_marys(stack, tables, 1, 62)
}

pub fn make_chals(stack: &mut NockStack, sample: Noun) -> Result {
    let [proof_noun, num_chals] = sample.uncell()?;
    let num_chals = num_chals.as_atom()?.as_u64()? as usize;
    let proof = Proof::try_from(proof_noun)?;
    let mut rng = absorb_proof_objects_impl(&proof.objects, &proof.hashes);
    Ok(rng.belts(num_chals).to_noun(stack))
}

pub fn weld_exts(stack: &mut NockStack, sample: Noun) -> Result {
    let [l, _, r_mary] = sample.uncell()?;
    let [l_header, l_mary] = l.uncell()?;

    let welded = weld_marys_step(stack, l_mary, r_mary)?;
    Ok(T(stack, &[l_header, welded]))
}

pub fn weld_table_marys(stack: &mut NockStack, sample: Noun) -> Result {
    let [ts, ms] = sample.uncell()?;
    let ts = HoonList::try_from(ts)?;
    let ms = HoonList::try_from(ms)?;
    let mut welded_tables = Vec::with_capacity(ts.count() + ms.count());
    for (t, m) in ts.zip(ms) {
        let [l, q_r] = t.uncell()?;
        let [l_header, l_mary] = l.uncell()?;
        let [_, r_mary] = m.uncell()?;

        let welded_mary = weld_marys_step(stack, l_mary, r_mary)?;
        let welded_table_mary = T(stack, &[l_header, welded_mary]);

        welded_tables.push(T(stack, &[welded_table_mary, q_r]));
    }
    Ok(welded_tables.to_noun(stack))
}

pub fn make_deep_weights(stack: &mut NockStack, sample: Noun) -> Result {
    let [proof, tables, max_constraint_degree] = sample.uncell()?;
    let proof = Proof::try_from(proof)?;
    let tables = HoonList::try_from(tables)?;
    let max_constraint_degree = max_constraint_degree.as_atom()?.as_u64()?;

    let total_cols: u64 = tables
        .map(|t| {
            let p = t.as_cell()?.head();
            let mary = p.as_cell()?.tail();
            let step = mary.as_cell()?.head();
            step.as_atom()?.as_u64()
        })
        .fold(0u64, |acc, x| acc + x.unwrap());

    let felts = absorb_proof_objects_impl(&proof.objects, &proof.hashes)
        .felts((total_cols * 4 + max_constraint_degree) as usize)
        .to_noun(stack);
    init_fpoly_bridge(stack, felts)
}

pub fn make_omicrons(stack: &mut NockStack, tables: Noun) -> Result {
    let tables = HoonList::try_from(tables)?;
    let mut omicrons = Vec::with_capacity(tables.count());
    let mut lifted_omicrons = Vec::with_capacity(tables.count());
    for t in tables {
        let table_mary = t.as_cell()?.head();
        let mary = MarySlice::try_from(table_mary.as_cell()?.tail()).map_err(|_| BAIL_FAIL)?;
        let omicron_belt = Belt(height_mary(mary) as u64).ordered_root()?;
        omicrons.push(omicron_belt.to_noun(stack));
        lifted_omicrons.push(Felt::lift(omicron_belt).to_noun(stack));
    }

    let omicrons = omicrons.to_noun(stack);
    let lifted_omicrons = lifted_omicrons.to_noun(stack);
    let bpolys = init_bpoly_bridge(stack, omicrons)?;
    let fpolys = init_fpoly_bridge(stack, lifted_omicrons)?;
    Ok(T(stack, &[bpolys, fpolys]))
}

pub fn get_max_constraint_degree(_stack: &mut NockStack, sample: Noun) -> Result {
    let map_iter = HoonMapIter::from(sample);

    let mut max_degree = 0u64;
    for entry in map_iter {
        let [_, value] = entry.uncell()?;
        let [boundary, row, transition, terminal, _extra] = value.uncell()?;
        let boundary = boundary.as_atom()?.as_u64()?;
        let row = row.as_atom()?.as_u64()?;
        let transition = transition.as_atom()?.as_u64()?;
        let terminal = terminal.as_atom()?.as_u64()?;

        max_degree = max_degree
            .max(boundary)
            .max(row)
            .max(transition)
            .max(terminal);
    }

    Ok(D(max_degree))
}

pub fn make_composition_poly(stack: &mut NockStack, sample: Noun) -> Result {
    let [proof, omicrons, heights, tworow_trace_polys, constraint_map, count_map, challenges, dyn_list, is_extra] =
        sample.uncell()?;

    let proof = Proof::try_from(proof)?;
    let omicrons = BPolySlice::try_from(omicrons)?;
    let heights = HoonList::try_from(heights)?
        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
        .collect::<Vec<_>>();
    let tworow_trace_polys = HoonList::try_from(tworow_trace_polys)?
        .map(|v| BPolySlice::try_from(v).unwrap())
        .collect::<Vec<_>>();
    let constraint_map = HoonMap::try_from(constraint_map).ok();
    let count_map = HoonMap::try_from(count_map)?;
    let challenges = BPolySlice::try_from(challenges)?;
    let dyn_list = HoonList::try_from(dyn_list)?
        .map(|v| BPolySlice::try_from(v).unwrap())
        .collect::<Vec<_>>();
    let is_extra = is_extra.as_direct()?.data() == 0;

    let weights_vec: Vec<Noun> = {
        let num_tables = heights.len();
        let mut num_constraints = 0usize;
        let mut counts = Vec::with_capacity(num_tables);
        for i in 0..num_tables {
            let total = count_map
                .get(stack, D(i as _))
                .ok_or(BAIL_FAIL)?
                .uncell::<5>()?
                .iter()
                .take(if is_extra { 5 } else { 4 })
                .map(|v| v.as_atom().unwrap().as_u64().unwrap())
                .sum::<u64>() as usize;
            counts.push(total);
            num_constraints += total;
        }
        let all_weights =
            absorb_proof_objects_impl(&proof.objects, &proof.hashes).belts(2 * num_constraints);

        let mut result = vec![];
        let mut offset = 0usize;
        for count in counts {
            let slice_len = 2 * count;
            let (res_atom, res_poly): (IndirectAtom, &mut [Belt]) =
                new_handle_mut_slice(stack, Some(slice_len));
            res_poly.copy_from_slice(&all_weights[offset..offset + slice_len]);
            result.push(finalize_poly(stack, Some(slice_len), res_atom));
            offset += slice_len;
        }
        result
    };

    // copied from eight::compute_composition_poly

    type Elem = Melt;

    // =/  max-height=@
    //   %-  bex  %-  xeb  %-  dec
    //   (roll heights max)
    let Some(&max_height) = heights.iter().max() else {
        return Err(BAIL_FAIL);
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
    let mut boundary_acc: Option<Vec<_>> = None;

    // Substitution moved out from process_degree_constraints to have everything done in one go.
    let mut engine = SubstituteEngine::new(max_height);
    let mut comp_cnts = Vec::with_capacity(1024);
    let tworow_trace_polys = tworow_trace_polys
        .iter()
        .map(|v| PolyVec(v.0.to_vec()))
        .map(<PolyVec<Elem>>::from)
        .collect::<Vec<_>>();
    for i in 0..omicrons.len() {
        crate::codefuscate! {
            // =/  trace  (snag i tworow-trace-polys)
            let trace = &tworow_trace_polys[i];
            let trace = <_ as Into<PolySlice<Elem>>>::into(trace);
            // =/  constraints  (~(got by constraint-w-deg-map.dp) i)
            let constraints2 = constraint_w_deg_map.get(&(i as u64)).unwrap();
            // =/  dyns  (snag i dyn-list)
            let dyns = dyn_list[i];

            for constraints in constraints2 {
                for (_, mp) in constraints.iter() {
                    // =/  comps=(list bpoly)
                    //   (mp-substitute-ultra mp trace max-height chal-map dyns)
                    comp_cnts.push(mp_substitute_ultra_impl(
                        &mut engine, 0, *mp, trace, challenges, dyns,
                    )?);
                }
            }
        }
    }

    let (all_comps, poly_len) = engine.reduce();
    let mut all_comps = all_comps.iter().flat_map(|m| m.chunks(poly_len));
    let mut comp_cnts = comp_cnts.into_iter();

    // ::
    // %+  roll  (range len.omicrons)
    // |=  [i=@ acc=_zero-bpoly]
    let mut acc = PolyVec(vec![Elem::zero(); poly_len]);
    for i in 0..omicrons.len() {
        crate::codefuscate! {
        // =/  height=@  (snag i heights)
        let height = heights[i];
        // =/  omicron  (~(snag bop omicrons) i)
        let omicron = omicrons.0[i];
        // =/  last-row  (init-bpoly ~[(bneg (binv omicron)) 1])      ::  f(X)=X-g^{-1}
        let last_row = [Elem::from_u64(bneg(binv(omicron.0))), Elem::one()];
        let last_row = PolySlice(&last_row);
        // =/  weights  (~(got by weights-map) i)
        let weights = BPolySlice::try_from(weights_vec[i])?;
        // =/  counts  (~(got by count_map) i)
        let counts = count_map.get(stack, D(i as _)).ok_or(BAIL_FAIL)?;
        let counts = counts
            .uncell::<5>()?
            .map(|v| v.as_atom().unwrap().as_u64().unwrap());
        // =/  constraints  (~(got by constraint-w-deg-map.dp) i)
        let constraints2 = constraint_w_deg_map.get(&(i as u64)).unwrap();
        // ::
        // =/  row-zerofier                                           ::  f(X) = (X^N-1)
        //   (bpsub (bppow id-bpoly height) one-bpoly)
        let row_zerofier = ppow(&[Elem::zero(), Elem::one()], height as _);
        let row_zerofier = psub_(&row_zerofier, &[Elem::one()]);
        let row_zerofier = PolySlice(&row_zerofier);
        let mut row_acc = Option::<Vec<_>>::None;

        // ::  note: the transition zerofier = row-zerofier/last-row
        // ::  here, we are computing composition-constraints/transition-zerofier
        let transition_zerofier = pdiv(row_zerofier.0, last_row.0);
        let transition_zerofier = PolySlice(&transition_zerofier);

        let dividends =
            [boundary_zerofier, row_zerofier, transition_zerofier, last_row, row_zerofier];

        let mut weights = weights.0;
        for (o, ((constraints, count), dividend)) in constraints2
            .iter()
            .zip(counts.into_iter())
            .zip(dividends)
            .enumerate()
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
            // ;:  bpadd
            //   acc

            match o {
                0 => match boundary_acc.as_mut() {
                    Some(acc) => padd_in_place(acc, &processed_constraints.0),
                    None => boundary_acc = Some(processed_constraints.0),
                },
                1 => match row_acc.as_mut() {
                    Some(acc) => padd_in_place(acc, &processed_constraints.0),
                    None => row_acc = Some(processed_constraints.0),
                },
                4 => match row_acc.as_mut() {
                    Some(acc) => padd_in_place(acc, &processed_constraints.0),
                    None => row_acc = Some(processed_constraints.0),
                },
                _ => {
                    let dividend = PolyVec(dividend.0.to_vec());
                    let result = pdiv(&processed_constraints.0, &dividend.0);
                    padd_in_place(&mut acc.0, &result);
                }
            }
        }

        if let Some(row_acc) = row_acc {
            let row_result = pdiv(&row_acc, row_zerofier.0);
            padd_in_place(&mut acc.0, &row_result);
        } }
    }

    if let Some(boundary_acc) = boundary_acc {
        let row_result = pdiv(&boundary_acc, boundary_zerofier.0);
        padd_in_place(&mut acc.0, &row_result);
    }

    let acc: BPolyVec = acc.into();

    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc.0);
    let ret = finalize_poly(stack, Some(acc.len()), ret);

    Ok(ret)
}

pub fn make_trace_evals(stack: &mut NockStack, sample: Noun) -> Result {
    let [tworow_trace_polys, eval_point] = sample.uncell()?;
    let marys = HoonList::try_from(tworow_trace_polys)?;
    let eval_point = eval_point.as_felt().copied()?;
    let mut polys = vec![];
    for mary in marys {
        let len = MarySlice::try_from(mary.as_cell()?)
            .map_err(|_| BAIL_FAIL)?
            .len as usize;
        for i in 0..len {
            let b = BPolySlice::try_from(snag_as_bpoly(stack, mary, i)?)?;
            polys.push(bpeval_lift(b, eval_point));
        }
    }

    let (res, res_poly): (IndirectAtom, &mut [Felt]) =
        new_handle_mut_slice(stack, Some(polys.len()));
    for (i, felt) in polys.iter().enumerate() {
        res_poly[i] = *felt;
    }
    Ok(finalize_poly(stack, Some(res_poly.len()), res))
}

fn build_merk_proof_impl(stack: &mut NockStack, m: Noun, axis: u64) -> Result {
    if axis == 0 {
        return Err(BAIL_FAIL);
    }
    fn rec(stack: &mut NockStack, merk_heap: Noun, axis: u64) -> Result {
        if axis == 0 {
            return Ok(D(0));
        }
        let sibling = if axis % 2 == 1 { axis + 1 } else { axis - 1 };
        let sibling_digest = snag_as_digest(stack, merk_heap, sibling as usize)?;
        let rest = rec(stack, merk_heap, (axis - 1) / 2)?;
        Ok(T(stack, &[sibling_digest, rest]))
    }
    rec(stack, m, axis - 1)
}

pub fn build_merk_proof(stack: &mut NockStack, sample: Noun) -> Result {
    let [merk, axis] = sample.uncell()?;
    let [root, m] = merk.uncell()?;
    let axis = axis.as_atom()?.as_u64()?;
    let proof = build_merk_proof_impl(stack, m, axis)?;
    Ok(T(stack, &[root, proof]))
}

pub fn add_commitments(stack: &mut NockStack, sample: Noun) -> Result {
    let [proof, fri_indices, commitments] = sample.uncell()?;
    let mut proof = Proof::try_from(proof)?;
    let fri_indices = HoonList::try_from(fri_indices)?;
    let commitments = HoonList::try_from(commitments)?;
    for idx in fri_indices {
        let idx = idx.as_atom()?.as_u64()?;
        for c in commitments {
            let [_, codewords, merk] = c.uncell()?;
            let [i, merk_heap] = merk.uncell()?;
            let [_, m] = merk_heap.uncell()?;
            let i = i.as_atom()?.as_u64()?;

            let mary = MarySlice::try_from(codewords).map_err(|_| BAIL_FAIL)?;
            let snagged = snag_one(stack, codewords, idx as usize)?;
            let arr = T(stack, &[D(mary.step as u64), snagged]);
            let axis = (1 << (i - 1)) + idx;
            let path = build_merk_proof_impl(stack, m, axis)?;
            let proof_data = ProofData::MPathBf(T(stack, &[arr, path]).try_into()?);
            proof.push(proof_data);
        }
    }
    Ok(proof.to_noun(stack))
}

pub fn weld_terminals(stack: &mut NockStack, dyn_list: Noun) -> Result {
    let dyn_list = HoonList::try_from(dyn_list)?.map(|v| BPolySlice::try_from(v).unwrap());
    let mut acc = vec![];
    for p in dyn_list {
        acc.extend_from_slice(&p.0);
    }
    let (ret, handle) = new_handle_mut_slice(stack, Some(acc.len()));
    handle.copy_from_slice(&acc);
    let ret = finalize_poly(stack, Some(acc.len()), ret);
    Ok(ret)
}

pub fn make_second_row_trace_polys(stack: &mut NockStack, sample: Noun) -> Result {
    let tables = HoonList::try_from(sample)?;
    let mut res = Vec::with_capacity(tables.count());
    for t in tables {
        let mary_noun = t.as_cell()?.head().as_cell()?.tail();
        let mary = MarySlice::try_from(mary_noun).map_err(|_| BAIL_FAIL)?;
        let polys_noun = transpose_bpolys(stack, mary)?;
        let len = MarySlice::try_from(polys_noun).map_err(|_| BAIL_FAIL)?.len as usize;
        if len == 0 {
            return Err(BAIL_FAIL);
        }

        let mut bpolys = Vec::with_capacity(len);
        for i in 0..len {
            let bp = snag_as_bpoly(stack, polys_noun, i)?;
            let shift_sam = T(stack, &[bp, D(1)]);
            let shifted = bp_shift_by_unity_sam(stack, shift_sam)?;
            let iffted = bp_ifft(BPolyVec::try_from(shifted)?)?;
            bpolys.push(iffted);
        }

        // zing-bpolys
        let step = bpolys[0].0.len();
        let (ret, mary) = new_handle_mut_mary(stack, step, len);
        let mut offset = 0;
        for bp in bpolys {
            let dat = bp.0.iter().map(|b| b.0 as u64).collect::<Vec<u64>>();
            mary.dat[offset..offset + step].copy_from_slice(&dat);
            offset += step;
        }
        let bpolys = finalize_mary(stack, step, len, ret);

        res.push(bpolys);
    }
    Ok(res.to_noun(stack))
}

pub fn make_trace_polys(stack: &mut NockStack, sample: Noun) -> Result {
    let [base, ext, mega_ext] = sample.uncell()?;
    let base = HoonList::try_from(base)?;
    let ext = HoonList::try_from(ext)?;
    let mega_ext = HoonList::try_from(mega_ext)?;

    let len_hint = base.count().min(ext.count()).min(mega_ext.count());
    let mut res = Vec::with_capacity(len_hint);

    for ((bm, em), mem) in base.zip(ext).zip(mega_ext) {
        let bm = MarySlice::try_from(bm).map_err(|_| BAIL_FAIL)?;
        let em = MarySlice::try_from(em).map_err(|_| BAIL_FAIL)?;
        let mem = MarySlice::try_from(mem).map_err(|_| BAIL_FAIL)?;

        if bm.step != em.step || bm.step != mem.step {
            return Err(BAIL_FAIL);
        }

        let step = bm.step as usize;
        let len = bm.len as usize + em.len as usize + mem.len as usize;
        let (ret, mary) = new_handle_mut_mary(stack, step, len);

        let mut offset = 0;
        let copy_into = |dst: &mut [u64], offset: &mut usize, src: &[u64]| {
            let end = *offset + src.len();
            dst[*offset..end].copy_from_slice(src);
            *offset = end;
        };

        copy_into(mary.dat, &mut offset, bm.dat);
        copy_into(mary.dat, &mut offset, em.dat);
        copy_into(mary.dat, &mut offset, mem.dat);

        let welded = finalize_mary(stack, step, len, ret);
        res.push(welded);
    }

    Ok(res.to_noun(stack))
}

pub fn make_composition_codewords(stack: &mut NockStack, sample: Noun) -> Result {
    let [composition_pieces, fri_domain_len] = sample.uncell()?;
    let composition_pieces = HoonList::try_from(composition_pieces)?;
    let fri_domain_len = fri_domain_len.as_atom()?.as_u64()? as u32;

    let root = Belt(fri_domain_len as u64)
        .ordered_root()
        .map_err(|_| BAIL_FAIL)?;

    let step = fri_domain_len as usize;
    let (ret, mary) = new_handle_mut_mary(stack, step, composition_pieces.count());
    let mut offset = 0;

    for piece in composition_pieces {
        let piece = BPolySlice::try_from(piece).map_err(|_| BAIL_FAIL)?;

        let codeword = bp_coseword(piece.0, &G, fri_domain_len, &root);
        if codeword.len() != step {
            return Err(BAIL_FAIL);
        }
        for (i, belt) in codeword.iter().enumerate() {
            mary.dat[offset + i] = belt.0;
        }
        offset += step;
    }

    Ok(finalize_mary(stack, step, composition_pieces.count(), ret))
}

pub fn make_deep_challenge(stack: &mut NockStack, sample: Noun) -> Result {
    let [proof_noun, n_noun] = sample.uncell()?;
    let proof = Proof::try_from(proof_noun)?;
    let n = n_noun.as_atom()?.as_u64()?;

    let mut rng = absorb_proof_objects_impl(&proof.objects, &proof.hashes);
    let exp_offset = Felt::lift(Belt(bpow(G.0, n)));

    loop {
        let deep_candidate = rng.felt();
        let mut exp_deep_can = Felt::zero();
        fpow(&deep_candidate, n, &mut exp_deep_can);

        if exp_deep_can != Felt::one() && exp_deep_can != exp_offset {
            return Ok(deep_candidate.to_noun(stack));
        }
    }
}

pub fn make_composition_piece_evals(stack: &mut NockStack, sample: Noun) -> Result {
    let [deep_challenge_noun, composition_pieces] = sample.uncell()?;
    let deep_challenge = deep_challenge_noun.as_felt().copied()?;
    let composition_pieces = HoonList::try_from(composition_pieces)?;

    let mut pieces = Vec::with_capacity(composition_pieces.count());
    for piece in composition_pieces {
        pieces.push(FPolySlice::try_from(piece).map_err(|_| BAIL_FAIL)?);
    }

    let mut c = Felt::zero();
    fpow(&deep_challenge, pieces.len() as u64, &mut c);

    let (ret, out): (IndirectAtom, &mut [Felt]) = new_handle_mut_slice(stack, Some(pieces.len()));
    for (i, fp) in pieces.iter().enumerate() {
        out[i] = peval::<Felt>(*fp, c);
    }

    Ok(finalize_poly(stack, Some(out.len()), ret))
}
