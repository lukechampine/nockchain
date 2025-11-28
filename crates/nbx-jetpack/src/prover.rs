use nockchain_math::belt::Belt;
use nockchain_math::felt::Felt;
use nockchain_math::mary::MarySlice;
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::structs::{HoonList, HoonMapIter};
use nockvm::jets::util::{slot, BAIL_FAIL};
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::{Noun, D, T};
use noun_serde::NounEncode;
use zkvm_jetpack::jets::bp_jets::init_bpoly_bridge;
use zkvm_jetpack::jets::fp_jets::init_fpoly_bridge;

use crate::four::{absorb_proof_objects_impl, Proof};
use crate::one::weld_marys_step;
use crate::seven::height_mary;

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

pub fn make_deep_weights(stack: &mut NockStack, subject: Noun) -> Result {
    let [proof, tables, max_constraint_degree] = subject.uncell()?;
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

pub fn make_comp_weights(stack: &mut NockStack, subject: Noun) -> Result {
    let [proof, num_constraints] = subject.uncell()?;
    let proof = Proof::try_from(proof)?;
    let num_constraints = num_constraints.as_atom()?.as_u64()?;

    let belts = absorb_proof_objects_impl(&proof.objects, &proof.hashes)
        .belts((2 * num_constraints) as usize)
        .to_noun(stack);

    init_bpoly_bridge(stack, belts)
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
