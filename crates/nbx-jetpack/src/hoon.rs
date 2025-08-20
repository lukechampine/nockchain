use std::cmp::Ordering;

use nockapp::Noun;
use nockvm::interpreter::Context;
use nockvm::jets::util::slot;
use nockvm::jets::Result;
use nockvm::noun::*;
use nockvm::site::{site_slam, Site};

pub fn list_sort(context: &mut Context, subject: Noun) -> Result {
    let sample = slot(subject, 6)?;
    let mut list = slot(sample, 2)?;
    let mut gate = slot(sample, 3)?;

    let mut list_vec = vec![];
    while let Ok(list_cell) = list.as_cell() {
        list_vec.push(list_cell.head());
        list = list_cell.tail();
    }

    // Since the gate doesn't change, we can do a single jet check and use that through the whole
    // loop
    let site = Site::new(context, &mut gate);

    let mut err = None;

    list_vec.sort_by(|a, b| {
        if err.is_some() {
            return Ordering::Less;
        }
        let arg = Cell::new(&mut context.stack, *a, *b);
        let flag = match site_slam(context, &site, arg.as_noun()) {
            Ok(f) => f,
            Err(e) => {
                err = Some(e);
                return Ordering::Less;
            }
        };

        if unsafe { flag.raw_equals(&YES) } {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });

    if let Some(e) = err {
        return Err(e);
    }

    list_vec.push(D(0));

    Ok(T(&mut context.stack, &list_vec))
}
