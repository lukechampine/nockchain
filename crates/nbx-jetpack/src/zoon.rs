use nockvm::interpreter::Context;
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::{Noun, Slots, T};
use zkvm_jetpack::noun::noun_ext::NounExt;

fn zby_key_impl(stack: &mut NockStack, map: Noun) -> Result {
    if map.is_atom() {
        return Ok(map);
    }

    let [n, l, r] = map.uncell()?;
    let [k, _] = n.uncell()?;

    #[rustfmt::skip]
    let l = if l.is_cell() { zby_key_impl(stack, l)? } else { l };
    #[rustfmt::skip]
    let r = if r.is_cell() { zby_key_impl(stack, r)? } else { r };

    Ok(T(stack, &[k, l, r]))
}

pub fn zby_key(context: &mut Context, subj: Noun) -> Result {
    let map = subj.slot(7)?.slot(6)?;
    zby_key_impl(&mut context.stack, map)
}
