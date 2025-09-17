use nockvm::interpreter::Context;
use nockvm::jets::util::slot;
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::*;
use zkvm_jetpack::form::{Belt, ElementEx, Felt};
use zkvm_jetpack::hand::structs::HoonList;
use zkvm_jetpack::jets::utils::jet_err;
use zkvm_jetpack::noun::noun_ext::NounExt;

// +$  pelt-stack
//   $:  alf=pelt
//       alf-inv=pelt
//       len=@
//       dat=pelt
//   ==
struct PolyStack<T> {
    alf: T,
    alf_inv: T,
    len: usize,
    dat: T,
}

impl<T: ElementEx> TryFrom<Noun> for PolyStack<T> {
    type Error = JetErr;

    fn try_from(value: Noun) -> std::result::Result<Self, Self::Error> {
        let [alf, alf_inv, len, dat] = value.uncell()?;
        let Ok(alf) = T::try_from(alf) else {
            return jet_err();
        };
        let Ok(alf_inv) = T::try_from(alf_inv) else {
            return jet_err();
        };
        let len = len.as_direct()?.data() as usize;
        let Ok(dat) = T::try_from(dat) else {
            return jet_err();
        };

        Ok(Self {
            alf,
            alf_inv,
            len,
            dat,
        })
    }
}

impl<T: ElementEx> PolyStack<T> {
    pub fn as_noun(&self, stack: &mut NockStack) -> Noun {
        let r = [
            self.alf.as_noun(stack),
            self.alf_inv.as_noun(stack),
            D(self.len as _),
            self.dat.as_noun(stack),
        ];
        T(stack, &r)
    }

    pub fn push(&mut self, x: T) {
        // ^-  pelt-stack
        // ps(len +(len.ps), dat (padd (pmul dat.ps alf.ps) x))
        self.len += 1;
        self.dat = (self.dat * self.alf) + x;
    }
}

fn poly_stack_push<T: ElementEx>(stack: &mut NockStack, ps: Noun, x: T) -> Result {
    // ::    [a b c] =>  [a b c x]
    // ~/  %push
    // |=  x=pelt
    let mut ps = PolyStack::<T>::try_from(ps)?;
    ps.push(x);
    Ok(ps.as_noun(stack))
}

pub fn poly_stack_push_raw<T: ElementEx>(context: &mut Context, subject: Noun) -> Result {
    let parent_core = slot(subject, 7)?;
    let ps = slot(parent_core, 6)?;
    let x = slot(subject, 6)?;
    let Ok(x) = T::try_from(x) else {
        return jet_err();
    };
    poly_stack_push(&mut context.stack, ps, x)
}
pub fn bstack_push(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_raw::<Belt>(context, subject)
}

pub fn fstack_push(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_raw::<Felt>(context, subject)
}

pub fn pstack_push(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_raw::<Felt>(context, subject)
}

fn poly_stack_push_all<T: ElementEx>(
    stack: &mut NockStack,
    ps: Noun,
    xs: impl Iterator<Item = T>,
) -> Result {
    // ::    [a b c] => [a b c x1 ... xn]
    // ~/  %push-all
    // |=  xs=(list pelt)
    // ^-  pelt-stack
    let mut ps = PolyStack::<T>::try_from(ps)?;
    // %+  roll  xs
    // |=  [x=pelt ps-new=_ps]
    // (~(push pstack ps-new) x)
    xs.for_each(|x| ps.push(x));
    Ok(ps.as_noun(stack))
}

pub fn poly_stack_push_all_raw<T: ElementEx>(context: &mut Context, subject: Noun) -> Result {
    let parent_core = slot(subject, 7)?;
    let ps = slot(parent_core, 6)?;
    let xs = slot(subject, 6)?;
    let xs = HoonList::try_from(xs).ok().into_iter().flatten();
    let xs =
        xs.map(|x| T::try_from(x).unwrap_or_else(|_| panic!("Unable to convert to rust datatype")));
    poly_stack_push_all(&mut context.stack, ps, xs)
}

pub fn bstack_push_all(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_all_raw::<Belt>(context, subject)
}

pub fn fstack_push_all(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_all_raw::<Felt>(context, subject)
}

pub fn pstack_push_all(context: &mut Context, subject: Noun) -> Result {
    poly_stack_push_all_raw::<Felt>(context, subject)
}

// ++  pop
//   ::    [a b c x] => [a b c]
//   ~/  %pop
//   |=  x=pelt
//   ^-  pelt-stack
//   ?>  (gth len.ps 0)
//   ps(len (dec len.ps), dat (pmul (psub dat.ps x) alf-inv.ps))
