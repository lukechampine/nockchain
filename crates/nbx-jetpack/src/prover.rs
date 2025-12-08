use nbx_tip5::base::{binv, bneg};
use nbx_tip5::melt::Melt;
use nockchain_math::belt::{bpow, Belt};
use nockchain_math::bpoly::bp_coseword;
use nockchain_math::felt::{finv_, fpow, Felt};
use nockchain_math::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use nockchain_math::mary::{mary_weld, MarySlice, MarySliceMut};
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::poly::{BPolySlice, *};
use nockchain_math::poly_ext::*;
use nockchain_math::structs::{HoonList, HoonMap, HoonMapIter};
use nockvm::jets::util::{slot, BAIL_FAIL};
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{IndirectAtom, Noun, D, T};
use noun_serde::NounEncode;
use zkvm_jetpack::form::gen_trace::build_tree_data;
use zkvm_jetpack::form::poly::Poly;
use zkvm_jetpack::jets::bp_jets::init_bpoly_bridge;
use zkvm_jetpack::jets::fp_jets::{coseword_sam, init_fpoly_bridge};
use zkvm_jetpack::jets::mary_jets::{snag_as_bpoly, snag_as_digest, snag_one, transpose_bpolys};

use super::substitute::SubstituteEngine;
use super::two::*;
use crate::eight::{
    compute_codeword_commitments_sam, compute_deep, degree_processing, precompute_ntts,
    process_composition_constraints,
};
use crate::engine::Engine;
use crate::four::{absorb_proof_objects_impl, digest, Proof, ProofData};
use crate::one::{weld_marys_step, G};
use crate::seven::height_mary;
use crate::three::bp_build_merk_heap;
use crate::utils::xeb;

fn collect_marys(
    stack: &mut NockStack,
    tables: &Vec<Noun>,
    table_mary_axis: u64,
    width_axis: u64,
) -> Result {
    let mut marys = Vec::with_capacity(tables.len());
    let mut width = 0u64;
    for t in tables {
        let table_mary = slot(*t, table_mary_axis)?;
        let header = slot(table_mary, 2)?;
        width += slot(header, width_axis)?.as_atom()?.as_u64()?;
        marys.push(slot(table_mary, 3)?);
    }

    let marys = marys.to_noun(stack);
    Ok(T(stack, &[marys, D(width)]))
}

fn bas_mary(stack: &mut NockStack, tables: &Vec<Noun>) -> Result {
    collect_marys(stack, tables, 2, 14)
}

fn ext_mary(stack: &mut NockStack, tables: &Vec<Noun>) -> Result {
    collect_marys(stack, tables, 1, 30)
}

fn mega_ext_mary(stack: &mut NockStack, tables: &Vec<Noun>) -> Result {
    collect_marys(stack, tables, 1, 62)
}

fn weld_table_marys(
    stack: &mut NockStack,
    ts: &Vec<Noun>,
    ms: &Vec<Noun>,
) -> std::result::Result<Vec<Noun>, JetErr> {
    let mut welded_tables = Vec::with_capacity(ts.len());
    for (t, m) in ts.iter().zip(ms.iter()) {
        let [l, q_r] = t.uncell()?;
        let [l_header, l_mary] = l.uncell()?;
        let [_, r_mary] = m.uncell()?;

        let welded_mary = weld_marys_step(stack, l_mary, r_mary)?;
        let welded_table_mary = T(stack, &[l_header, welded_mary]);

        welded_tables.push(T(stack, &[welded_table_mary, q_r]));
    }

    Ok(welded_tables)
}

fn make_composition_poly(
    stack: &mut NockStack,
    proof: &Proof,
    omicrons: BPolySlice,
    heights: &Vec<u64>,
    tworow_trace_polys: &Vec<BPolySlice>,
    constraint_map: HoonMap,
    count_map: HoonMap,
    challenges: BPolySlice,
    dyn_list: &Vec<BPolySlice>,
    is_extra: bool,
) -> std::result::Result<BPolyVec, JetErr> {
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
        degree_processing(stack, &heights, Some(constraint_map), is_extra)?;
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

    Ok(acc.into())
}

fn make_trace_evals(
    stack: &mut NockStack,
    tworow_trace_polys: Noun,
    eval_point: Felt,
) -> std::result::Result<FPolyVec, JetErr> {
    let marys = HoonList::try_from(tworow_trace_polys)?;
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
    Ok(PolyVec(polys))
}

fn build_merk_proof(stack: &mut NockStack, m: Noun, axis: u64) -> Result {
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
            let path = build_merk_proof(stack, m, axis)?;
            let proof_data = ProofData::MPathBf(T(stack, &[arr, path]).try_into()?);
            proof.push(proof_data);
        }
    }
    Ok(proof.to_noun(stack))
}

pub fn giant_chunk_v2(stack: &mut NockStack, sample: Noun) -> Result {
    // |=  $:  =proof
    //         pre=preprocess-data
    //         base-tables=(list table-dat)
    //         return=fock-return
    //         s=*
    //         f=*
    //     ==
    let [proof, pre, base_tables, ret, s, f] = sample.uncell()?;
    let base_tables = HoonList::try_from(base_tables)?.collect::<Vec<Noun>>();

    _ = f; // NOTE: unused!

    // =/  heights  (table-heights base-tables)
    let heights = {
        let mut heights = Vec::with_capacity(base_tables.len());
        for t in &base_tables {
            // extract mary len without decoding everything
            let mary_len = slot(*t, 0b10110)?.as_atom()?.as_u64()?;
            let height = if mary_len == 0 {
                0
            } else {
                mary_len.next_power_of_two()
            };
            heights.push(height);
        }
        heights
    };
    let max_height = *heights.iter().max().unwrap();
    let max_padded_height = max_height.next_power_of_two();

    // =.  proof  (~(push proof-stream proof) [%heights heights])
    let mut proof = Proof::try_from(proof)?;
    proof.push(ProofData::Heights(heights.to_noun(stack)));

    // =/  fri-domain-len=@  ~(fri-domain-len calc heights cd.pre)
    let fri_domain_len = max_padded_height << 6;

    // =/  base=codeword-commitments
    //   =/  [base-marys=(list mary) width=@]  (bas-mary base-tables)
    //   (compute-codeword-commitments base-marys fri-domain-len width)
    let base = {
        let [base_marys, width] = bas_mary(stack, &base_tables)?.uncell()?;
        let sub_sam = T(stack, &[base_marys, D(fri_domain_len), width]);
        compute_codeword_commitments_sam(stack, sub_sam)?
    };

    // =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.base])
    proof.push(ProofData::MRoot(digest(slot(base, 0b11110)?)?));

    // =/  chals-rd1=(list belt)  (make-chals proof num-chals-rd1:chal)
    let chals_rd1 = absorb_proof_objects_impl(&proof.objects, &proof.hashes).belts(14 * 3); // (lent chal-names-rd1))

    // =/  table-exts=(list table-mary)  (make-table-exts base-tables chals-rd1 return)
    let table_exts = {
        let extend_funcs = &[
            zkvm_jetpack::jets::memory_table_jets_v2::memory_v2_extend_sam,
            zkvm_jetpack::jets::compute_table_jets_v2::compute_v2_extend_sam,
        ];
        assert_eq!(base_tables.len(), extend_funcs.len());
        let chals = chals_rd1.to_noun(stack);
        let mut res = Vec::with_capacity(base_tables.len());
        for (t, extend) in base_tables.iter().zip(extend_funcs.iter()) {
            let [table_mary, _, _] = t.uncell()?;

            let sam = T(stack, &[table_mary, chals, ret]);
            res.push(extend(stack, sam)?);
        }

        res
    };

    // =^  [ext=codeword-commitments mega-ext=codeword-commitments all-tables=(list table-dat) augmented-chals=bpoly]  proof
    //   %-  medium-chunk
    //   :*  proof
    //       base-tables
    //       table-exts
    //       fri-domain-len
    //       chals-rd1
    //       num-chals-rd2:chal
    //       return
    //       s
    //       f
    //   ==

    // =/  ext-tables  (weld-table-marys base-tables table-exts)
    let ext_tables = weld_table_marys(stack, &base_tables, &table_exts)?;

    // =/  ext=codeword-commitments
    //   =/  [ext-marys=(list mary) width=@]  (ext-mary table-exts)
    //   (compute-codeword-commitments ext-marys fri-domain-len width)
    let ext = {
        let [ext_marys, width] = ext_mary(stack, &table_exts)?.uncell()?;
        let sub_sam = T(stack, &[ext_marys, D(fri_domain_len), width]);
        compute_codeword_commitments_sam(stack, sub_sam)?
    };

    // =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.ext])
    proof.push(ProofData::MRoot(digest(slot(ext, 0b11110)?)?));

    // =/  challenges  (weld chals-rd1 (make-chals proof num-chals-rd2))
    let challenges = {
        let chals_rd2 = absorb_proof_objects_impl(&proof.objects, &proof.hashes).belts(12 * 3); // (lent chal-names-rd2)
        let mut chals = Vec::with_capacity(chals_rd1.len() + chals_rd2.len());
        chals.extend_from_slice(&chals_rd1);
        chals.extend_from_slice(&chals_rd2);
        chals
    };

    // =/  table-mega-exts=(list table-mary)  (build-mega-extend ext-tables challenges return)
    let table_mega_exts = {
        let chals = challenges.to_noun(stack);

        let mega_extend_funcs = &[
            zkvm_jetpack::jets::memory_table_jets_v2::memory_v2_mega_extend_sam,
            zkvm_jetpack::jets::compute_table_jets_v2::compute_v2_mega_extend_sam,
        ];

        assert_eq!(ext_tables.len(), mega_extend_funcs.len());

        let mut res = Vec::with_capacity(ext_tables.len());
        for (t, mega_extend) in ext_tables.iter().zip(mega_extend_funcs.iter()) {
            let [table_mary, _, _] = t.uncell()?;

            let sam = T(stack, &[table_mary, chals, ret]);
            res.push(mega_extend(stack, sam)?);
        }

        res
    };

    // =/  all-tables  (weld-table-marys ext-tables table-mega-exts)
    let all_tables = weld_table_marys(stack, &ext_tables, &table_mega_exts)?;

    // =/  mega-ext=codeword-commitments
    //   =/  [mega-ext-marys=(list mary) width=@]  (mega-ext-mary table-mega-exts)
    //   (compute-codeword-commitments mega-ext-marys fri-domain-len width)
    let mega_ext = {
        let [mega_ext_marys, width] = mega_ext_mary(stack, &table_mega_exts)?.uncell()?;
        let sub_sam = T(stack, &[mega_ext_marys, D(fri_domain_len), width]);
        compute_codeword_commitments_sam(stack, sub_sam)?
    };

    // =/  augmented-chals=bpoly  (augment-challenges:chal challenges s f)
    let augmented_chals = {
        const NUM_CHALS: usize = (14 + 12) * 3; // (lent chal-names-basic)

        let got_pelt = |index: usize| {
            let offset = index * 3;
            Felt([challenges[offset], challenges[offset + 1], challenges[offset + 2]])
        };
        let a = got_pelt(0);
        let b = got_pelt(1);
        let c = got_pelt(2);
        let alf = got_pelt(13);

        let inv_alf = finv_(&alf);
        let tree_data = build_tree_data(s, &alf)?;
        let input_ifp = a * tree_data.size + b * tree_data.dyck + c * tree_data.leaf;

        let mut augmented = vec![];
        augmented.extend_from_slice(&challenges[..NUM_CHALS]);
        augmented.extend_from_slice(&inv_alf.0);
        augmented.extend_from_slice(&input_ifp.0);

        let augmented_list = augmented.to_noun(stack);
        BPolySlice::try_from(init_bpoly_bridge(stack, augmented_list)?)?
    };

    // =/  dyn-list=(list bpoly)  (make-dyn-list all-tables)
    let dyn_list = {
        let terminal_funcs = &[
            zkvm_jetpack::jets::memory_table_jets_v2::memory_v2_terminal_sam,
            zkvm_jetpack::jets::compute_table_jets_v2::compute_v2_terminal_sam,
        ];

        assert_eq!(all_tables.len(), terminal_funcs.len());

        let mut res = Vec::with_capacity(all_tables.len());
        for (t, terminal) in all_tables.iter().zip(terminal_funcs.iter()) {
            let [table_mary, _, _] = t.uncell()?;
            let bp = BPolySlice::try_from(terminal(stack, table_mary)?)?;
            res.push(bp);
        }

        res
    };

    // =.  proof  (~(push proof-stream proof) terms+(weld-terminals dyn-list))
    proof.push(ProofData::Terms({
        let mut acc = vec![];
        for bp in &dyn_list {
            acc.extend_from_slice(&bp.0);
        }
        PolyVec(acc)
    }));

    // =^  [deep-codeword=fpoly commitments=(list codeword-commitments)]  proof
    //   %-  big-chunk
    //   :*  proof
    //       base
    //       ext
    //       mega-ext
    //       pre
    //       all-tables
    //       heights
    //       augmented-chals
    //       dyn-list
    //       fri-domain-len
    //   ==

    let [cd_pre, constraint_map_pre, count_map_pre] = pre.uncell()?;
    let cd_pre = HoonMapIter::from(cd_pre);
    let constraint_map_pre = HoonMap::try_from(constraint_map_pre)?;
    let count_map_pre = HoonMap::try_from(count_map_pre)?;

    //     =/  trace-polys=(list mary)  (make-trace-polys polys.base polys.ext polys.mega-ext)
    let trace_polys = {
        let base = HoonList::try_from(base.as_cell().unwrap().head())?;
        let ext = HoonList::try_from(ext.as_cell().unwrap().head())?;
        let mega_ext = HoonList::try_from(mega_ext.as_cell().unwrap().head())?;

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

        res.to_noun(stack)
    };

    //     =/  tworow-trace-polys=(list mary)  (make-tworow-trace-polys trace-polys all-tables)
    let tworow_trace_polys = {
        let trace_polys = HoonList::try_from(trace_polys)?;

        let second_row = {
            let mut res = Vec::with_capacity(all_tables.len());
            for t in &all_tables {
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
            res
        };

        let len = trace_polys.count().min(second_row.len());
        let mut res = Vec::with_capacity(len);
        for (t_poly, s_poly) in trace_polys.zip(second_row.into_iter()) {
            let t_poly = MarySlice::try_from(t_poly).map_err(|_| BAIL_FAIL)?;
            let s_poly = MarySlice::try_from(s_poly).map_err(|_| BAIL_FAIL)?;

            let ret_len = (t_poly.len + s_poly.len) as usize;
            let (ret, mary) = new_handle_mut_mary(stack, t_poly.step as usize, ret_len);

            let mut offset = 0;
            mary.dat[offset..offset + (t_poly.len as usize * t_poly.step as usize)]
                .copy_from_slice(t_poly.dat);
            offset += t_poly.len as usize * t_poly.step as usize;
            mary.dat[offset..offset + (s_poly.len as usize * s_poly.step as usize)]
                .copy_from_slice(s_poly.dat);

            res.push(finalize_mary(stack, t_poly.step as usize, ret_len, ret));
        }

        res.to_noun(stack)
    };

    //     =/  max-constraint-degree  (get-max-constraint-degree cd.pre)
    let max_constraint_degree = {
        let mut max_degree = 0u64;
        for entry in cd_pre {
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
        max_degree
    };

    //     =/  tworow-trace-polys-eval=(list bpoly)  (make-tworow-trace-polys-eval tworow-trace-polys max-constraint-degree (roll heights max))
    let tworow_trace_polys_eval = {
        let tworow_trace_polys = HoonList::try_from(tworow_trace_polys)?;
        if max_constraint_degree == 0 || max_height == 0 {
            return Err(BAIL_FAIL);
        }

        let ntt_len = max_constraint_degree.next_power_of_two();

        let mut res = Vec::with_capacity(tworow_trace_polys.count());
        for polys in tworow_trace_polys {
            let sam = T(stack, &[polys, D(max_padded_height), D(ntt_len)]);
            let ntts = precompute_ntts(stack, sam)?;
            res.push(BPolySlice::try_from(ntts)?);
        }

        res
    };

    //     =/  [omicrons-bpoly=bpoly omicrons-fpoly=fpoly]  (make-omicrons all-tables)
    let (omicrons_bpoly, omicrons_fpoly) = {
        let mut omicrons = Vec::with_capacity(all_tables.len());
        let mut lifted_omicrons = Vec::with_capacity(all_tables.len());
        for t in &all_tables {
            let table_mary = t.as_cell()?.head();
            let mary = MarySlice::try_from(table_mary.as_cell()?.tail()).map_err(|_| BAIL_FAIL)?;
            let omicron_belt = Belt(height_mary(mary) as u64).ordered_root()?;
            omicrons.push(omicron_belt.to_noun(stack));
            lifted_omicrons.push(Felt::lift(omicron_belt).to_noun(stack));
        }

        let omicrons = omicrons.to_noun(stack);
        let lifted_omicrons = lifted_omicrons.to_noun(stack);
        let bpoly = BPolySlice::try_from(init_bpoly_bridge(stack, omicrons)?)?;
        let fpoly = init_fpoly_bridge(stack, lifted_omicrons)?;
        (bpoly, fpoly)
    };

    //     =/  extra-composition-poly=bpoly
    //       %-  make-composition-poly
    //       :*  proof
    //           omicrons-bpoly
    //           heights
    //           tworow-trace-polys-eval
    //           constraint-map.pre
    //           count-map.pre
    //           augmented-chals
    //           dyn-list
    //           %.y
    //       ==
    //     =.  proof  (~(push proof-stream proof) [%poly extra-composition-poly])
    proof.push(ProofData::Poly(make_composition_poly(
        stack, &proof, omicrons_bpoly, &heights, &tworow_trace_polys_eval, constraint_map_pre,
        count_map_pre, augmented_chals, &dyn_list, true,
    )?));

    //     =/  extra-comp-eval-point=felt
    //         =/  rng  ~(prover-fiat-shamir proof-stream proof)
    //         =^  f  rng  $:felt:rng
    //         f
    let extra_comp_eval_point = absorb_proof_objects_impl(&proof.objects, &proof.hashes).felt();

    //     =/  extra-trace-evaluations=fpoly  (make-trace-evals tworow-trace-polys extra-comp-eval-point)
    let extra_trace_evaluations =
        make_trace_evals(stack, tworow_trace_polys, extra_comp_eval_point)?;

    //     =.  proof  (~(push proof-stream proof) [%evals extra-trace-evaluations])
    proof.push(ProofData::Evals(extra_trace_evaluations.clone()));

    //     =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.mega-ext])
    let m_root = {
        let [_, _, mega_ext_merk] = mega_ext.uncell()?;
        let [_, mega_ext_merk_heap] = mega_ext_merk.uncell()?;
        let [mega_ext_merk_root, _] = mega_ext_merk_heap.uncell()?;
        ProofData::MRoot(digest(mega_ext_merk_root)?)
    };
    proof.push(m_root);

    //     =/  composition-pieces=(list bpoly)
    //       =/  composition-poly=bpoly
    //         %-  make-composition-poly
    //         :*  proof
    //             omicrons-bpoly
    //             heights
    //             tworow-trace-polys-eval
    //             constraint-map.pre
    //             count-map.pre
    //             augmented-chals
    //             dyn-list
    //             %.n
    //         ==
    //       (bp-decompose composition-poly max-constraint-degree)
    let composition_pieces = {
        let composition_poly = make_composition_poly(
            stack, &proof, omicrons_bpoly, &heights, &tworow_trace_polys_eval, constraint_map_pre,
            count_map_pre, augmented_chals, &dyn_list, false,
        )?;
        p_decompose(
            PolySlice(composition_poly.data()),
            max_constraint_degree as usize,
        )
    };

    //     =/  composition-codeword-array=mary  (transpose-bpolys (make-composition-codewords composition-pieces fri-domain-len))
    let composition_codeword_array = {
        let root = Belt(fri_domain_len as u64)
            .ordered_root()
            .map_err(|_| BAIL_FAIL)?;

        let step = fri_domain_len as usize;
        let (ret, mary) = new_handle_mut_mary(stack, step, composition_pieces.len());
        let mut offset = 0;

        for piece in &composition_pieces {
            let codeword = bp_coseword(piece, &G, fri_domain_len as u32, &root);
            if codeword.len() != step {
                return Err(BAIL_FAIL);
            }
            for (i, belt) in codeword.iter().enumerate() {
                mary.dat[offset + i] = belt.0;
            }
            offset += step;
        }
        let codewords = finalize_mary(stack, step, composition_pieces.len(), ret);
        let codewords = MarySlice::try_from(codewords).map_err(|_| BAIL_FAIL)?;
        transpose_bpolys(stack, codewords)?
    };

    //     =/  composition-merk  (bp-build-merk-heap:merkle composition-codeword-array)
    let composition_merk = bp_build_merk_heap(stack, composition_codeword_array)?;

    //     =.  proof  (~(push proof-stream proof) [%comp-m h.q.composition-merk max-constraint-degree])
    let comp_m = {
        let [_, comp_heap] = composition_merk.uncell()?;
        ProofData::CompM(digest(comp_heap.as_cell()?.head())?, max_constraint_degree)
    };
    proof.push(comp_m);

    //     =/  deep-challenge=felt  (make-deep-challenge proof fri-domain-len)
    let deep_challenge = {
        let mut rng = absorb_proof_objects_impl(&proof.objects, &proof.hashes);
        let exp_offset = Felt::lift(Belt(bpow(G.0, fri_domain_len)));
        loop {
            let deep_candidate = rng.felt();
            let mut exp_deep_can = Felt::zero();
            fpow(&deep_candidate, fri_domain_len, &mut exp_deep_can);

            if exp_deep_can != Felt::one() && exp_deep_can != exp_offset {
                break deep_candidate;
            }
        }
    };

    //     =/  trace-evaluations=fpoly  (make-trace-evals tworow-trace-polys deep-challenge)
    let trace_evaluations = make_trace_evals(stack, tworow_trace_polys, deep_challenge)?;

    //     =/  composition-pieces-fpoly  (turn composition-pieces bpoly-to-fpoly)
    let composition_pieces_fpoly = {
        let mut fpolys = Vec::with_capacity(composition_pieces.len());
        for piece in &composition_pieces {
            fpolys.push(bpoly_to_fpoly(PolySlice(piece.data())));
        }
        fpolys.to_noun(stack)
    };

    //     =/  composition-piece-evaluations=fpoly  (make-composition-piece-evals deep-challenge composition-pieces-fpoly)
    let composition_piece_evaluations = {
        let composition_pieces_fpoly = HoonList::try_from(composition_pieces_fpoly)?;
        let mut pieces = Vec::with_capacity(composition_pieces_fpoly.count());
        for piece in composition_pieces_fpoly {
            pieces.push(FPolySlice::try_from(piece).map_err(|_| BAIL_FAIL)?);
        }

        let mut c = Felt::zero();
        fpow(&deep_challenge, pieces.len() as u64, &mut c);

        let (ret, out): (IndirectAtom, &mut [Felt]) =
            new_handle_mut_slice(stack, Some(pieces.len()));
        for (i, fp) in pieces.iter().enumerate() {
            out[i] = peval::<Felt>(*fp, c);
        }

        finalize_poly(stack, Some(out.len()), ret)
    };

    //     =.  proof  (~(push proof-stream proof) [%evals trace-evaluations])
    proof.push(ProofData::Evals(trace_evaluations.clone()));

    //     =.  proof  (~(push proof-stream proof) [%evals composition-piece-evaluations])
    proof.push(ProofData::Evals(FPolyVec::try_from(
        composition_piece_evaluations,
    )?));

    //     =/  deep-codeword=fpoly
    //       =/  deep-poly=fpoly
    //         %-  compute-deep
    //         :*  trace-polys
    //             (~(weld fop trace-evaluations) extra-trace-evaluations)
    //             composition-pieces-fpoly
    //             composition-piece-evaluations
    //             (make-deep-weights proof all-tables max-constraint-degree)
    //             omicrons-fpoly
    //             deep-challenge
    //             extra-comp-eval-point
    //         ==
    //       (coseword deep-poly (lift g) fri-domain-len)
    let deep_codeword = {
        let evals = {
            let mut evals =
                Vec::with_capacity(trace_evaluations.len() + extra_trace_evaluations.len());
            evals.extend_from_slice(trace_evaluations.data());
            evals.extend_from_slice(extra_trace_evaluations.data());
            PolyVec(evals)
        };

        let deep_weights = {
            let total_cols: u64 = all_tables
                .into_iter()
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
            init_fpoly_bridge(stack, felts)?
        };

        let deep_poly = {
            let evals = evals.to_noun(stack);
            let deep_challenge = deep_challenge.to_noun(stack);
            let eval_point = extra_comp_eval_point.to_noun(stack);
            let sub_sam = T(
                stack,
                &[
                    trace_polys, evals, composition_pieces_fpoly, composition_piece_evaluations,
                    deep_weights, omicrons_fpoly, deep_challenge, eval_point,
                ],
            );
            compute_deep(stack, sub_sam)?
        };

        let lift_g = Felt::lift(G).to_noun(stack);
        let sub_sam = T(stack, &[deep_poly, lift_g, D(fri_domain_len)]);
        coseword_sam(stack, sub_sam)?
    };

    //     =/  commitments  ~[base ext mega-ext [~ composition-codeword-array composition-merk]]
    let commitments = {
        let comp_commitment = T(stack, &[D(0), composition_codeword_array, composition_merk]);
        vec![base, ext, mega_ext, comp_commitment].to_noun(stack)
    };

    // [[heights deep-codeword commitments] proof]
    let heights = heights.to_noun(stack);
    let head = T(stack, &[heights, deep_codeword, commitments]);
    let proof = proof.to_noun(stack);
    Ok(T(stack, &[head, proof]))
}
