use std::iter::once;
use std::mem::MaybeUninit;

use crate::form::mary::{Mary, MarySlice};
use crate::form::math::tip5::{self, CAPACITY, DIGEST_LENGTH, RATE, STATE_SIZE};
use crate::form::tip5::permute;
use crate::form::{
    mont_reduction, montify, BPolyVec, Element, ElementEx, FPolySlice, Felt, Melt, PolySlice,
    PolyVec,
};
use crate::form::{poly::Poly, BPolySlice, Belt};
use crate::hand::handle::{
    finalize_mary, finalize_poly, new_handle_mut_mary, new_handle_mut_slice,
};
use crate::hand::structs::HoonList;
use crate::noun::noun_ext::NounExt;
use array_concat::concat_arrays;
use either::Either;
use nockvm::interpreter::Context;
use nockvm::jets::util::slot;
use nockvm::jets::{util::BAIL_EXIT, JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, DirectAtom, Noun, D, T};
use nockvm_macros::tas;

use tracing::log::*;

use crate::jets::utils::jet_err;

use super::one::*;
use super::utils::*;

pub fn leaf_sequence(stack: &mut NockStack, t: Noun) -> Result {
    let mut r = leaf_sequence_impl(t)?;
    r.push(D(0));
    Ok(T(stack, &r))
}

trait FromAtom: Sized {
    fn from_atom(a: Atom) -> core::result::Result<Self, JetErr>;
}

impl FromAtom for Atom {
    fn from_atom(a: Atom) -> core::result::Result<Self, JetErr> {
        Ok(a)
    }
}

impl FromAtom for Noun {
    fn from_atom(a: Atom) -> core::result::Result<Self, JetErr> {
        Ok(a.as_noun())
    }
}

impl FromAtom for Belt {
    fn from_atom(a: Atom) -> core::result::Result<Self, JetErr> {
        Ok(Belt(a.as_u64()?))
    }
}

fn leaf_sequence_impl<T: FromAtom>(mut t: Noun) -> core::result::Result<Vec<T>, JetErr> {
    // ~/  %leaf-sequence
    // |=  t=*
    // %-  flop
    // ^-  (list @)
    // =|  vec=(list @)
    // |-
    // ?@  t  t^vec
    // $(t +.t, vec $(t -.t))

    // NOTE: Let's do this differently...
    // leaf-sequence constructs a flattened reversed list of all elems of t (as a list!), and then
    // reverses it. So, let's just flatten in-order...
    let mut ret = vec![];
    let mut prev: Vec<Noun> = vec![];

    loop {
        match t.as_either_atom_cell() {
            Either::Left(a) => {
                // if t = atom:

                //   push t to cur.head
                ret.push(T::from_atom(a)?);

                //   t = prev.pop_cell() else break
                let Some(prev_t) = prev.pop() else { break };
                t = prev_t;
            }
            Either::Right(c) => {
                // else:

                //  prev.push_cell(cell.tail)
                prev.push(c.tail());

                //  t = cell.head
                t = c.head();
            }
        }
    }

    Ok(ret)
}

pub fn init_tip5_state(domain: u64) -> [Melt; STATE_SIZE] {
    match domain {
        // ^~((reap state-size 0))
        tas!(b"variable") => [Melt(0); STATE_SIZE],
        // ^~((weld (reap rate 0) (reap capacity (montify 1))))
        tas!(b"fixed") => {
            let zero = [Melt(0); RATE];
            let mont = [Melt::one(); CAPACITY];
            concat_arrays!(zero, mont)
        }
        _ => panic!("Unsupported tip5 state"),
    }
}

pub fn new_sponge(variable: bool) -> [Melt; tip5::STATE_SIZE] {
    let mode = if variable {
        tas!(b"variable")
    } else {
        tas!(b"fixed")
    };
    init_tip5_state(mode)
}

pub fn absorb_sponge<const PAD: bool, T: Into<Melt> + Copy>(
    sponge: &mut [Melt; tip5::STATE_SIZE],
    input: &[T],
) {
    // |=  input=(list belt)
    // ^+  +>.$
    // =*  rng  +>.$

    // |^
    // ::  assert that input is made of base field elements
    // ?>  (levy input based)

    // =/  [q=@ r=@]  (dvr (lent input) rate)
    let l = input.len();
    let q = l / RATE;
    let r = l % RATE;

    // ::  pad input with ~[1 0 ... 0] to be a multiple of rate
    // =.  input  (weld input [1 (reap (dec (sub rate r)) 0)])
    // ::  bring input into montgomery space
    // =.  input  (turn input montify)
    let (input, end) = input.split_at(RATE * q);
    let input = input
        .chunks_exact(RATE)
        .map(|i| {
            <[T; RATE]>::try_from(i)
                .unwrap()
                .map(<T as Into<Melt>>::into)
        })
        .chain(if PAD {
            let mut r = [MaybeUninit::uninit(); RATE];
            let (a, b) = r.split_at_mut(end.len());
            a.iter_mut()
                .zip(
                    end.iter()
                        .copied()
                        .map(<T as Into<Melt>>::into)
                        .map(MaybeUninit::new),
                )
                .for_each(|(a, b)| *a = b);
            let (a, b) = b.split_at_mut(1);
            a[0] = MaybeUninit::new(Melt::one());
            b.iter_mut()
                .for_each(|v| *v = MaybeUninit::new(Melt::zero()));
            Some(r.map(|v| unsafe { MaybeUninit::assume_init(v) }))
        } else {
            assert_eq!(r, 0);
            None
        });

    // |-
    // ?:  =(q 0)
    //   rng
    for input_head in input {
        // =.  sponge  (absorb-rate (scag rate input))

        // ++  absorb-rate
        //   ?>  =((lent input) rate)
        //   =.  sponge  (weld input (slag rate sponge))
        sponge[..RATE].copy_from_slice(&input_head);
        //   $:permute
        tip5::permute(sponge);
    }
}

pub fn squeeze_sponge(spo: [Melt; tip5::STATE_SIZE]) -> [Melt; RATE] {
    // |.  ^+  [*(list belt) +.$]
    // =*  rng  +.$
    // ::  squeeze out the full rate and bring out of montgomery space
    // =/  output  (turn (scag rate sponge) mont-reduction)
    let ret = <[Melt; RATE]>::try_from(&spo[..RATE]).unwrap();
    // NOTE: we do not permute the sponge, because that's inefficient
    // =.  sponge  $:permute
    ret
}

pub fn hash_ten_cell(stack: &mut NockStack, sam: Noun) -> Result {
    // |=  =ten-cell
    let input: [Noun; 2] = sam.uncell()?;
    let input: [[Noun; 5]; 2] = input.map(|v| v.uncell().unwrap());
    // SAFETY: we are merely transposing the array
    let input: [Noun; 10] = unsafe { core::mem::transmute(input) };
    let input = input
        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
        .map(Melt::from_u64);
    // ^-  noun-digest
    let output = hash_10(input);
    let output = output
        .map(Belt::from)
        .map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &output))
}

pub fn hash_10_sam(stack: &mut NockStack, sam: Noun) -> Result {
    // ::  +hash-10: hash list of 10 belts into a list of 5 belts
    // |=  input=(list belt)
    let input: [Noun; 11] = sam.uncell()?;
    let input: [Noun; 10] = input[..10].try_into().unwrap();
    let input = input
        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
        .map(Melt::from_u64);
    // ::  output length is 5
    // ^-  (list belt)
    let output = hash_10(input);
    let output = output
        .map(Belt::from)
        .map(|v| Atom::new(stack, v.0).as_noun());
    let output: [Noun; 6] = concat_arrays!(output, [D(0)]);
    Ok(T(stack, &output))
}

pub fn hash_10(input: [Melt; 10]) -> NounDigest {
    // Verify that this list has length 10 and all elems are direct:
    // ?>  =((lent input) rate)
    // ?>  (levy input based)
    // FIXME: acc verify this

    // =.  input   (turn input montify)
    // let input = input.map(|v| montify(v.0));
    // =/  sponge  (init-tip5-state %fixed)
    // =.  sponge  (permutation (weld input (slag rate sponge)))
    // (turn (scag digest-length sponge) mont-reduction)

    hash_any::<false, _>(&input)
}

pub fn hash_noun_varlen(stack: &mut NockStack, sam: Noun) -> Result {
    // |=  n=*
    // ^-  noun-digest
    let mut engine = HashEngine::default();
    engine.push_noun(0, sam)?;
    let output = engine.reduce()[0]
        .map(Belt::from)
        .map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &output))
}

pub fn hash_varlen_sam(stack: &mut NockStack, sam: Noun) -> Result {
    // |=  input=(list belt)
    let input = HoonList::try_from(sam)
        .ok()
        .into_iter()
        .flatten()
        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
        .map(Melt::from_u64)
        .collect::<Vec<_>>();
    // ^-  (list belt)
    let output = hash_varlen(&input);
    let output = output
        .map(Belt::from)
        .map(|v| Atom::new(stack, v.0).as_noun());
    let output: [Noun; 6] = concat_arrays!(output, [D(0)]);
    Ok(T(stack, &output))
}

pub fn hash_varlen<T: Into<Melt> + Copy>(input: &[T]) -> NounDigest {
    hash_any::<true, T>(input)
}

pub fn hash_any<const PAD: bool, T: Into<Melt> + Copy>(input: &[T]) -> NounDigest {
    // |=  input=(list belt)
    // ^-  (list belt)
    // =/  spo  (new:sponge)
    let mut spo = new_sponge(PAD);

    // =.  spo  (absorb:spo input)
    absorb_sponge::<PAD, T>(&mut spo, input);

    // =^  output  spo
    //   (squeeze:spo)
    let output = squeeze_sponge(spo);

    // (scag digest-length output)
    output[..DIGEST_LENGTH].try_into().unwrap()
}

pub fn hash_pairs(inp: &[NounDigest]) -> core::result::Result<Vec<NounDigest>, JetErr> {
    // |=  lis=(list (list @))

    let mut ret = vec![];

    // NOTE: this loop essentially takes the input list, and gets its pairs, reducing the size in
    // half. That's the point of `++  indices`.
    for v in inp.chunks(2) {
        let Ok([first, second]) = <[NounDigest; 2]>::try_from(v) else {
            return jet_err();
        };
        // (hash-10:tip5 (weld (snag b lis) (snag +(b) lis)))
        // :: (weld <...>)
        let welded = concat_arrays!(first, second);
        // hash-10:tip5
        let hashed = hash_10(welded);
        ret.push(hashed);
    }

    Ok(ret)
}

pub struct Tip5Tog {
    pub sponge: [Melt; tip5::STATE_SIZE],
}

// NOTE: no to_noun, since this requires us to include the whole core
impl Tip5Tog {
    pub fn belts(&mut self, n: usize) -> Vec<Belt> {
        // |=  n=@
        // ^+  [*(list belt) +>.$]
        // =*  rng  +>.$
        // =/  sponge  ~(. sponge spo)
        // =/  [q=@ r=@]  (dvr n rate)
        let q = n / RATE;
        let r = n % RATE;

        // =|  output=(list belt)
        let mut output = vec![];

        // |-
        // =^  out  sponge
        //   (squeeze:sponge)
        // =.  spo  sponge:sponge
        // ?:  =(q 0)
        //   [(weld output (scag r out)) rng]
        // $(q (dec q), output (weld output out))
        for _ in 0..q {
            output.extend(squeeze_sponge(self.sponge).map(Belt::from));
            permute(&mut self.sponge);
        }
        let last = squeeze_sponge(self.sponge);
        permute(&mut self.sponge);
        output.extend(last[..r].iter().map(|v| Belt::from(*v)));

        output
    }

    pub fn felts(&mut self, n: usize) -> Vec<Felt> {
        let belts = self.belts(n * 3);
        let belts_ptr = belts.as_ptr();
        unsafe { core::slice::from_raw_parts(belts_ptr as *const Felt, n) }.to_vec()
    }

    pub fn felt(&mut self) -> Felt {
        let belts = self.belts(3);
        Felt(belts.try_into().unwrap())
    }
}

pub fn with_tog(
    context: &mut Context,
    subj: Noun,
    func: impl FnOnce(&mut NockStack, &mut Tip5Tog, Noun) -> Result,
) -> Result {
    let stack = &mut context.stack;

    let parent = slot(subj, 7)?;
    let [_2, _3] = parent.uncell()?;
    let [state, _7] = _3.uncell()?;

    let state: [Noun; tip5::STATE_SIZE + 1] = state.uncell()?;
    let state: [Noun; tip5::STATE_SIZE] = unsafe { core::mem::transmute_copy(&state) };
    let sponge = state.map(|v| Melt(v.as_atom().unwrap().as_u64().unwrap()));
    let mut tog = Tip5Tog { sponge };
    let sam = slot(subj, 6)?;

    let ret = func(stack, &mut tog, sam)?;

    let sponge: [Noun; tip5::STATE_SIZE] = tog.sponge.map(|v| Atom::new(stack, v.0).as_noun());
    let state: [Noun; tip5::STATE_SIZE + 1] = concat_arrays!(sponge, [D(0)]);
    let state = T(stack, &state);

    let _3 = T(stack, &[state, _7]);
    let parent = T(stack, &[_2, _3]);

    Ok(T(stack, &[ret, parent]))
}

pub fn tog_belts(context: &mut Context, subj: Noun) -> Result {
    with_tog(context, subj, |stack, tog, n| {
        let n = n.as_direct()?.data() as usize;
        let belts = tog.belts(n);
        let belts = belts
            .into_iter()
            .map(|b| Atom::new(stack, b.0).as_noun())
            .chain(once(D(0)))
            .collect::<Vec<_>>();
        Ok(T(stack, &belts))
    })
}

pub fn tog_felts(context: &mut Context, subj: Noun) -> Result {
    with_tog(context, subj, |stack, tog, n| {
        let n = n.as_direct()?.data() as usize;
        let felts = tog.felts(n);
        let felts = felts
            .into_iter()
            .map(|f| f.as_noun(stack))
            .chain(once(D(0)))
            .collect::<Vec<_>>();
        Ok(T(stack, &felts))
    })
}

pub type NounDigest<T = Melt> = [T; 5];

#[derive(Debug)]
enum ReduceTy {
    Variable(usize),
    Fixed,
}

#[derive(Debug)]
struct ReduceOp {
    source: usize,
    ty: ReduceTy,
    destination: usize,
}

impl ReduceOp {
    fn reduce(self, input: &[Melt], out_ptr: *mut Melt, out_len: usize) {
        let dig = match self.ty {
            ReduceTy::Variable(len) => hash_varlen(&input[self.source..(self.source + len)]),
            ReduceTy::Fixed => hash_10(
                input[self.source..(self.source + DIGEST_LENGTH * 2)]
                    .try_into()
                    .unwrap(),
            ),
        };

        assert!(out_len >= self.destination + DIGEST_LENGTH);
        unsafe { core::slice::from_raw_parts_mut(out_ptr.add(self.destination), DIGEST_LENGTH) }
            .copy_from_slice(&dig);
    }
}

#[derive(Default)]
struct ReduceStage {
    ops: Vec<ReduceOp>,
    out: Vec<Melt>,
}

impl ReduceStage {
    fn reduce(mut self, inp: &[Melt]) -> Vec<Melt> {
        // TODO: multithread/GPU this.
        //use rayon::prelude::*;
        struct MeltSlice(*mut Melt);
        unsafe impl Send for MeltSlice {}
        unsafe impl Sync for MeltSlice {}
        let out_ptr = MeltSlice(self.out.as_mut_ptr());
        let out_len = self.out.len();
        self.ops.into_iter().for_each(|op| {
            let out = &out_ptr;
            op.reduce(inp, out.0, out_len);
        });
        self.out
    }

    fn push_variable(&mut self, a: usize, len: usize) -> usize {
        let ret = self.out.len();
        self.out.resize(ret + DIGEST_LENGTH, Melt(0));
        self.ops.push(ReduceOp {
            source: a,
            ty: ReduceTy::Variable(len),
            destination: ret,
        });
        ret
    }

    fn push_fixed(&mut self, a: usize, b: usize) -> usize {
        assert_eq!(a + DIGEST_LENGTH, b);
        let ret = self.out.len();
        self.out.resize(ret + DIGEST_LENGTH, Melt(0));
        self.ops.push(ReduceOp {
            source: a,
            ty: ReduceTy::Fixed,
            destination: ret,
        });
        ret
    }
}

#[derive(Default)]
pub struct HashEngine {
    stages: Vec<ReduceStage>,
}

trait Pushable {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr>;
}

impl Pushable for Noun {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr> {
        engine.push(stage, self)
    }
}

impl<T: Into<Melt>> Pushable for NounDigest<T> {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr> {
        Ok(engine.push_hash(stage, self))
    }
}

impl HashEngine {
    pub fn push_varlen(&mut self, stage: usize, m: impl Iterator<Item = Melt>) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.out.len();
        stage.out.extend(m);
        ret
    }

    pub fn push_noun(&mut self, stage: usize, n: Noun) -> core::result::Result<usize, JetErr> {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        // ~/  %hash-noun-varlen
        // |=  n=*
        // ^-  noun-digest
        // =/  leaf=(list @)  (leaf-sequence:shape n)
        let leaf = leaf_sequence_impl::<Belt>(n)?;

        // =/  dyck=(list @)  (dyck:shape n)
        let dyck = dyck(n);

        // =/  size  (lent leaf)
        let size = leaf.len();

        // (hash-belts-list [size (weld leaf dyck)])
        let len = 1 + leaf.len() + dyck.len();

        let melts = [Belt(size as u64)]
            .into_iter()
            .chain(leaf)
            .chain(dyck)
            .map(Melt::from);

        let source = self.push_varlen(stage + 1, melts);
        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_variable(source, len);

        Ok(ret)
    }

    fn push_list_inner<T: Pushable>(
        &mut self,
        stage: usize,
        l: impl Iterator<Item = T>,
    ) -> core::result::Result<(usize, usize), JetErr> {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage0 = self.stages.get_mut(stage).unwrap();
        let ret = stage0.out.len();
        stage0.out.push(Melt(0));

        let mut next = ret + 1;
        let mut len = 0;
        for h in l {
            let out = h.push(self, stage)?;
            len += 1;
            assert_eq!(out, next);
            next = out + DIGEST_LENGTH;
        }

        let stage = self.stages.get_mut(stage).unwrap();
        stage.out[ret] = Melt::from_u64((len * 5 + 1) as _);
        // Push dyck shape
        stage.out.push(Melt::zero());
        let shape = [0, 0, 1, 0, 1, 0, 1, 0, 1, 1].map(Melt::from_u64);
        stage.out.extend(core::iter::repeat_n(shape, len).flatten());

        Ok((ret, len))
    }

    pub fn reduce(mut self) -> Vec<NounDigest> {
        let mut cur = vec![];
        //let mut cnt = 0;
        while let Some(stage) = self.stages.pop() {
            //println!("Layer {cnt}: {} {}", stage.ops.len(), stage.out.len());
            //cnt += 1;
            //let t = std::time::Instant::now();
            cur = stage.reduce(&cur);
            //println!("{:.02}s", t.elapsed().as_secs_f64())
        }
        assert_eq!(cur.len() % DIGEST_LENGTH, 0);
        let p = cur.as_mut_ptr();
        let l = cur.len() / DIGEST_LENGTH;
        let c = cur.capacity() / DIGEST_LENGTH;
        core::mem::forget(cur);
        unsafe { Vec::from_raw_parts(p as *mut NounDigest, l, c) }
    }

    pub fn push_mary(&mut self, stage: usize, ma: MarySlice) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        //   %-  hash-hashable

        //   :-  leaf+step.p.h
        let step = self.push_noun(stage + 1, D(ma.step as _)).unwrap();

        //   :-  leaf+len.array.p.h
        let len = self.push_noun(stage + 2, D(ma.len as _)).unwrap();

        //   hash+(hash-belts-list (bpoly-to-list array:(~(change-step ave p.h) 1)))
        let hash_src = self.push_varlen(stage + 3, ma.dat.iter().copied().map(Melt::from_u64));
        let stage2 = self.stages.get_mut(stage + 2).unwrap();
        let hash = stage2.push_variable(hash_src, ma.dat.len());

        let stage1 = self.stages.get_mut(stage + 1).unwrap();
        let arr = stage1.push_fixed(len, hash);

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_fixed(step, arr);

        ret
    }

    pub fn push_hash<T: Into<Melt>>(&mut self, stage: usize, h: NounDigest<T>) -> usize {
        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.out.len();

        let d = h.map(Into::into);

        stage.out.extend_from_slice(&d[..]);
        ret
    }

    pub fn push_list<T: Pushable>(
        &mut self,
        stage: usize,
        l: impl Iterator<Item = T>,
    ) -> core::result::Result<usize, JetErr> {
        let (source, len) = self.push_list_inner(stage + 1, l)?;

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_variable(source, 2 + (DIGEST_LENGTH + 10) * len);

        Ok(ret)
    }

    pub fn ensure_stages(&mut self, stage: usize) {
        while self.stages.len() <= stage {
            self.stages.push(ReduceStage::default());
        }
    }

    pub fn push_pair(&mut self, stage: usize, a: usize, b: usize) -> usize {
        let stage = self.stages.get_mut(stage).unwrap();
        stage.push_fixed(a, b)
    }

    pub fn push(&mut self, stage: usize, h: Noun) -> core::result::Result<usize, JetErr> {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let h = h.as_cell()?;
        let ty = h.head().as_direct().map(|v| v.data());

        match ty {
            Ok(tas!(b"hash")) => {
                // ?:  ?=(%hash -.h)
                //   p.h
                Ok(self.push_hash(
                    stage,
                    h.tail()
                        .uncell()?
                        .map(|v| Belt(v.as_atom().unwrap().as_u64().unwrap())),
                ))
            }
            Ok(tas!(b"leaf")) => {
                // ?:  ?=(%leaf -.h)
                //   (hash-noun-varlen p.h)
                self.push_noun(stage, h.tail())
            }
            Ok(tas!(b"list")) => {
                // ?:  ?=(%list -.h)
                //   (hash-noun-varlen (turn p.h hash-hashable))
                let l = HoonList::try_from(h.tail()).ok().into_iter().flatten();
                self.push_list(stage, l)
            }
            Ok(tas!(b"mary")) => {
                let Ok(ma) = MarySlice::try_from(h.tail()) else {
                    return jet_err();
                };
                Ok(self.push_mary(stage, ma))
            }
            _ => {
                // %-  hash-ten-cell
                // [$(h p.h) $(h q.h)]
                let a = self.push(stage + 1, h.head())?;
                let b = self.push(stage + 1, h.tail())?;
                Ok(self.push_pair(stage, a, b))
            }
        }
    }
}

pub fn hash_hashable(stack: &mut NockStack, h: Noun) -> Result {
    let mut engine = HashEngine::default();
    engine.push(0, h)?;
    let r = engine.reduce()[0];
    let r = r.map(Belt::from).map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &r))
}

pub fn dyck(t: Noun) -> Vec<Belt> {
    // ~/  %dyck
    // |=  t=*
    // %-  flop
    // ^-  (list @)
    // =|  vec=(list @)
    // |-
    // ?@  t  vec
    // $(t +.t, vec [1 $(t -.t, vec [0 vec])])
    // TODO: make this non-recursive
    fn recurse(t: Noun, vec: Vec<Belt>) -> Vec<Belt> {
        let Ok(t) = t.as_cell() else {
            return vec;
        };
        let mut head_vec = vec![Belt(0)];
        head_vec.extend_from_slice(&vec);
        let head_res = recurse(t.head(), head_vec);
        let mut tail_vec = vec![Belt(1)];
        tail_vec.extend_from_slice(&head_res);
        recurse(t.tail(), tail_vec)
    }
    let mut res = recurse(t, vec![]);
    res.reverse();
    res
}

pub fn bp_build_merk_heap(stack: &mut NockStack, ma: Noun) -> Result {
    let Ok(ma) = MarySlice::try_from(ma) else {
        return jet_err();
    };
    let (height, mh) = build_merk_heap_impl::<Belt>(ma)?;
    let height = Atom::new(stack, height as _).as_noun();
    let mh = mh.to_noun(stack);
    Ok(T(stack, &[height, mh]))
}

pub fn build_merk_heap(stack: &mut NockStack, ma: Noun) -> Result {
    let Ok(ma) = MarySlice::try_from(ma) else {
        return jet_err();
    };
    let (height, mh) = build_merk_heap_impl::<Felt>(ma)?;
    let height = Atom::new(stack, height as _).as_noun();
    let mh = mh.to_noun(stack);
    Ok(T(stack, &[height, mh]))
}

pub struct MerkHeap {
    pub h: NounDigest,
    pub m: Mary,
}

impl MerkHeap {
    pub fn to_noun(self, stack: &mut NockStack) -> Noun {
        let (ret, handle) = new_handle_mut_mary(stack, self.m.step as _, self.m.len as _);
        handle.dat.copy_from_slice(&self.m.dat);
        let ma = finalize_mary(stack, self.m.step as _, self.m.len as _, ret);
        let h = self
            .h
            .map(Belt::from)
            .map(|v| Atom::new(stack, v.0))
            .map(Atom::as_noun);
        let h = T(stack, &h);
        T(stack, &[h, ma])
    }
}

pub fn build_merk_heap_impl<T: ElementEx>(
    m: MarySlice,
) -> core::result::Result<(usize, MerkHeap), JetErr> {
    // Definitions:
    // +$  mary  [step=@ =array]
    //    An array where each element is step size (in u64 words). This can be used to build
    //    multi-dimensional arrays or to store any data you want in one contiguous array.
    // +$  merk-heap  [h=noun-digest:tip5 m=mary]
    //     Heap ordered merkle tree stored in an array ?
    //     +$  noun-digest  [belt belt belt belt belt]
    //          +$  belt  @
    //            An integer in the interval [0, p).
    //            Due to a well chosen p, almost all numbers representable with 64 bits
    //            are present in the interval. In other words, a belt under our choice
    //            of p will always fit in 64 bits.

    // ~/  %bp-build-merk-heap-hoon
    // |=  m=mary

    // ::
    // ::  +heapify-mary
    // ::  Take a mary of belts, merklize it, and return it as a heap
    // ++  heapify-mary
    //   |=  m=mary     :: take an array of belts
    //   ^-  mary       :: return type: merkelized array of belts
    //   =/  size  (dec (bex (xeb len.array.m)))
    let height = xeb(m.len as usize);
    let size: u32 = (1 << height) - 1;

    //   :: each digest is 5 64-bit atoms; multiplying rounded-up array size by 5; shifting 64 bits left that many times; alloc'ed memory for new array?
    //   =/  high-bit  (lsh [6 (mul size 5)] 1)
    // NOTE: high bit is handled by new_handle_mut_mary

    //   ::  make leaves
    //   =/  res=(list (list @))
    //     %+  turn
    //       (range len.array.m)
    //     |=  i=@
    //     =/  t  (~(snag-as-bpoly ave m) i)
    //     (leaf-sequence:shape (hash-hashable:tip5 (hashable-bpoly:tip5 t)))
    //let mut res_l = Vec::with_capacity(m.len as usize);
    let mut engine = HashEngine::default();
    for i in 0..m.len {
        let t = snag_as_poly_mary::<T>(m, i as usize);
        let hbp = hashable_poly(t);
        engine.push_mary(0, hbp);
    }
    let res_l = engine.reduce();
    assert_eq!(res_l.len(), m.len as usize);

    //   :+  5
    //     size
    //   %+  add
    //     high-bit
    //   %+  rep  6
    //   %-  zing
    //   ^-  (list (list @))
    //   =/  curr  res
    //   |-
    //   ?:  =((lent curr) 1)
    //     res
    //   =/  pairs  (hash-pairs:tip5 curr)  :: pair the list up
    //   %=  $
    //     res      (weld pairs res)
    //     curr     pairs
    //   ==
    // --
    //
    //      IS EQUIVALENT TO:
    //
    // loop over res;
    //   - split cloned list into pairs and hash
    //   - prepend hashes to front of res (this does the heaping)
    //   - loop until no more pairs can be made (i.e. height of heap)
    // ... then promote the lists into a single list
    // ... then assemble the list into an atom
    // ... then add the high bit to the result
    // ... then build a mary out of the result
    let mut res = vec![];
    let mut curr = res_l;
    loop {
        let osize = res.len();
        res.resize(osize + curr.len(), NounDigest::default());
        res.copy_within(0..osize, curr.len());
        res[..curr.len()].copy_from_slice(&curr);

        if curr.len() == 1 {
            break;
        }

        curr = hash_pairs(&curr)?;
    }
    assert_eq!(res.len(), size as usize);

    // :-  (xeb len.array.m)          :: compute height of heap
    // :-  %+  snag-as-digest:tip5      :: retrieve the 0th entry of the heap and return it
    //       heap-mary                  ::   as a tip5 hash digest
    //     0
    let digest = res[0];

    let rl = res.len() * 5;
    let rc = res.capacity() * 5;
    let r = res.as_mut_ptr() as *mut Melt;
    core::mem::forget(res);
    // SAFETY: NounDigest is 5 u64, and it's all contiguous, therefore safe to transmute.
    let res = unsafe { Vec::from_raw_parts(r, rl, rc) };
    let res: BPolyVec = PolyVec(res).into();
    // SAFETY: Same here
    let d = unsafe { core::slice::from_raw_parts(res.0.as_ptr() as *const u64, res.len()) };
    let m = Mary {
        step: 5,
        len: (res.len() / 5) as _,
        dat: d.to_vec(),
    };

    let merk_heap = MerkHeap { h: digest, m };

    // heap-mary
    Ok((height, merk_heap))
}

fn hashable_poly<'a, T: ElementEx>(p: PolySlice<'a, T>) -> MarySlice<'a> {
    let dat =
        unsafe { core::slice::from_raw_parts(p.0.as_ptr() as *const u64, p.0.len() * T::len()) };
    MarySlice {
        step: T::len() as _,
        len: p.len() as _,
        dat,
    }
}

fn snag_as_digest(stack: &mut NockStack, m: Noun, i: usize) -> Result {
    // ::  +snag-as-digest
    // ::  Retrieve the i-th entry of the mary return it as a tip5 hash digest.
    // ::  Assumes that each entry of the mary is a single hash encoded in base 64.
    // ::
    // ++  snag-as-digest
    //   ~/  %snag-as-digest
    //   |=  [m=mary i=@]
    //   ^-  noun-digest:tip5
    let Ok(ma) = MarySlice::try_from(m) else {
        return jet_err();
    };

    //   ?>  =(5 step.m)
    if ma.step != 5 {
        return Err(BAIL_EXIT);
    }

    //   =/  buf  (~(snag ave m) i)
    let buf = snag_mary(stack, ma, i);

    //   :*  (cut 6 [0 1] buf)
    //       (cut 6 [1 1] buf)
    //       (cut 6 [2 1] buf)
    //       (cut 6 [3 1] buf)
    //       (cut 6 [4 1] buf)
    //   ==
    let uno = cut(stack, 6, 0, 1, buf)?.as_noun();
    let dos = cut(stack, 6, 1, 1, buf)?.as_noun();
    let tre = cut(stack, 6, 2, 1, buf)?.as_noun();
    let qua = cut(stack, 6, 3, 1, buf)?.as_noun();
    let cin = cut(stack, 6, 4, 1, buf)?.as_noun();

    Ok(T(stack, &[uno, dos, tre, qua, cin]))
}
