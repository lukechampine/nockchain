use nockvm::interpreter::Context;
use nockvm::jets::util::slot;
use nockvm::jets::JetErr;
use nockvm::noun::{Atom, Noun, D, T};
use nockvm::mem::NockStack;

use crate::form::math::tip5::*;
use crate::form::Melt;
use crate::jets::utils::jet_err;

pub fn hoon_list_to_sponge(list: Noun) -> Result<[Melt; STATE_SIZE], JetErr> {
    if list.is_atom() {
        return jet_err();
    }

    let mut sponge = [Melt(0); STATE_SIZE];
    let mut current = list;
    let mut i = 0;

    while current.is_cell() {
        let cell = current.as_cell()?;
        sponge[i] = Melt(cell.head().as_atom()?.as_u64()?);
        current = cell.tail();
        i = i + 1;
    }

    if i != STATE_SIZE {
        return jet_err();
    }

    Ok(sponge)
}

pub fn vec_to_hoon_list(stack: &mut NockStack, vec: &[u64]) -> Noun {
    let mut list = D(0);
    for e in vec.iter().rev() {
        let n = Atom::new(stack, *e).as_noun();
        list = T(stack, &[n, list]);
    }
    list
}

pub fn permutation(stack: &mut NockStack, sample: Noun) -> Result<Noun, JetErr> {
    let mut sponge = hoon_list_to_sponge(sample)?;
    permute(&mut sponge);

    let new_sponge = vec_to_hoon_list(stack, &sponge.map(|v| v.0));

    Ok(new_sponge)
}

pub fn permutation_jet(context: &mut Context, subject: Noun) -> Result<Noun, JetErr> {
    let sample = slot(subject, 6)?;
    permutation(&mut context.stack, sample)
}
