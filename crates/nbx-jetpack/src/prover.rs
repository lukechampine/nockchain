use nockchain_math::structs::HoonList;
use nockvm::jets::util::slot;
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::{Noun, D};
use noun_serde::NounEncode;

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
