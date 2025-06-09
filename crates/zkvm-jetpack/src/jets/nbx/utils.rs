use crate::form::mary::MarySlice;
use crate::form::Element;
use crate::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use crate::jets::utils::jet_err;
use nockvm::jets::util;
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::mug::mug;
use nockvm::noun::*;
use nockvm::serialization::jam;

pub struct DP(pub Noun);

impl core::fmt::Debug for DP {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if let Ok(c) = self.0.as_cell() {
            write!(f, "{:?}", nockvm::noun::FullDebugCellDepth(&c, 3))
        } else {
            write!(f, "{:?}", self.0)
        }
    }
}

pub fn scag_mut<T>(a: usize, b: &mut [T]) -> &mut [T] {
    b.split_at_mut(core::cmp::min(a, b.len())).0
}

pub fn scag_ref<T>(a: usize, b: &[T]) -> &[T] {
    b.split_at(core::cmp::min(a, b.len())).0
}

pub fn slag_mut<T>(a: usize, b: &mut [T]) -> &mut [T] {
    b.split_at_mut(core::cmp::min(a, b.len())).1
}

pub fn slag_ref<T>(a: usize, b: &[T]) -> &[T] {
    b.split_at(core::cmp::min(a, b.len())).1
}

pub fn slag_vec<T>(a: usize, mut b: Vec<T>) -> Vec<T> {
    b.split_off(core::cmp::min(a, b.len()))
}

pub fn xeb(v: usize) -> usize {
    (usize::BITS - v.leading_zeros()) as usize
}

static mut JAM: bool = false;
//static mut JAMC: usize = 0;

pub fn jam_to2(stack: &mut NockStack, noun: Noun, p: &str) {
    if !unsafe { JAM } {
        return;
    }
    let dir = std::path::Path::new("jetjam");
    let p = dir.join(format!("{p}.jam"));
    std::fs::create_dir_all(dir);
    if !p.exists() {
        let out = jam(stack, noun);
        let b = out.as_ne_bytes();
        std::fs::write(p, b).unwrap()
    }
}

pub fn jam_to<T: Element + Copy>(stack: &mut NockStack, acc: &[T], p: &str) {
    if !unsafe { JAM } {
        return;
    }
    let dir = std::path::Path::new("jetjam");
    let f = dir.join(format!("{p}.jam"));
    if !f.exists() {
        let (res, res_poly): (IndirectAtom, &mut [T]) =
            new_handle_mut_slice(stack, Some(acc.len()));
        res_poly.copy_from_slice(acc);
        //println!("RES {acc:?} {res:?}");
        let res_cell = finalize_poly(stack, Some(acc.len()), res);
        jam_to2(stack, res_cell, p)
    }
}

/*fn enable_jam() {
    unsafe {
        JAMC += 1;
        if JAMC >= 2 {
            //println!("ENABLE JAM");
            JAM = true;
        }
    };
}*/
//fn jam_to2(_: &mut NockStack, _: Noun, _: &str) {}
//fn jam_to<T: Element + Copy>(_: &mut NockStack, _: &[T], _: &str) {}

pub fn vprint<T: Element + Copy>(stack: &mut NockStack, acc: &[T]) {
    let (res, res_poly): (IndirectAtom, &mut [T]) = new_handle_mut_slice(stack, Some(acc.len()));
    res_poly.copy_from_slice(acc);
    //println!("RES {acc:?} {res:?}");
    let res_cell = finalize_poly(stack, Some(acc.len()), res);
    println!("RC: {:?}", DP(res_cell))
    //mug(stack, res_cell).data()
}

pub fn vmug<T: Element + Copy>(stack: &mut NockStack, acc: &[T]) -> u64 {
    let (res, res_poly): (IndirectAtom, &mut [T]) = new_handle_mut_slice(stack, Some(acc.len()));
    res_poly.copy_from_slice(acc);
    //println!("RES {acc:?} {res:?}");
    let res_cell = finalize_poly(stack, Some(acc.len()), res);
    mug(stack, res_cell).data()
}

pub fn mmug(stack: &mut NockStack, m: &MarySlice) -> u64 {
    let (res, res_ma) = new_handle_mut_mary(stack, m.step as usize, m.len as usize);
    res_ma.dat.copy_from_slice(m.dat);
    let res_ma = finalize_mary(stack, m.step as usize, m.len as usize, res);
    mug(stack, res_ma).data()
}

pub fn cut_direct(
    bloq: usize,
    start: usize,
    run: usize,
    atom: DirectAtom,
) -> core::result::Result<DirectAtom, JetErr> {
    if run == 0 {
        return Ok(D(0).as_direct()?);
    }

    if util::bite_to_word(bloq, run)? > 1 {
        return jet_err();
    }

    let direct = unsafe {
        let mut new_direct = D(0).as_direct()?;
        let new_slice = new_direct.as_bitslice_mut();
        util::chop(bloq, start, run, 0, new_slice, atom.as_bitslice())?;
        new_direct
    };

    Ok(direct)
}

pub fn cut(
    stack: &mut NockStack,
    bloq: usize,
    start: usize,
    run: usize,
    atom: Atom,
) -> core::result::Result<Atom, JetErr> {
    if run == 0 {
        return Ok(Atom::new(stack, 0));
    }

    let new_indirect = unsafe {
        let (mut new_indirect, new_slice) =
            IndirectAtom::new_raw_mut_bitslice(stack, util::bite_to_word(bloq, run)?);
        util::chop(bloq, start, run, 0, new_slice, atom.as_bitslice())?;
        new_indirect.normalize_as_atom()
    };

    Ok(new_indirect)
}

pub fn reap(stack: &mut NockStack, size: usize, val: Noun) -> Result {
    produce_list(stack, 0, size, |_, _| val)
}

pub fn produce_list(
    stack: &mut NockStack,
    start: usize,
    end: usize,
    mut f: impl FnMut(&mut NockStack, usize) -> Noun,
) -> Result {
    let mut cur = D(0);

    for i in (start..end).rev() {
        let elem = f(stack, i);
        cur = Cell::new(stack, elem, cur).as_noun();
    }

    Ok(cur)
}
