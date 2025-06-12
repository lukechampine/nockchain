use std::mem::MaybeUninit;

use crate::form::mary::MarySlice;
use crate::form::math::tip5::{self, CAPACITY, DIGEST_LENGTH, RATE, STATE_SIZE};
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

pub fn hash_noun_varlen(n: Noun) -> core::result::Result<NounDigest, JetErr> {
    // ~/  %hash-noun-varlen
    // |=  n=*
    // ^-  noun-digest
    // =/  leaf=(list @)  (leaf-sequence:shape n)
    let leaf = leaf_sequence_impl::<Belt>(n)?;

    // =/  dyck=(list @)  (dyck:shape n)
    let dyck = dyck(n)?;

    // =/  size  (lent leaf)
    let size = leaf.len();

    // (hash-belts-list [size (weld leaf dyck)])
    let belts = [Belt(size as u64)]
        .into_iter()
        .chain(leaf)
        .chain(dyck)
        .collect::<Vec<_>>();

    Ok(hash_varlen(&belts))
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

pub fn hash_10(input: [Melt; 10]) -> NounDigest {
    // ::  +hash-10: hash list of 10 belts into a list of 5 belts
    // |=  input=(list belt)
    // ::  output length is 5
    // ^-  (list belt)

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

type NounDigest = [Melt; 5];

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
}

#[derive(Default)]
struct HashEngine {
    stages: Vec<ReduceStage>,
}

impl HashEngine {
    fn push_varlen(&mut self, stage: usize, m: impl Iterator<Item = Melt>) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.out.len();
        stage.out.extend(m);
        ret
    }

    fn push_noun(&mut self, stage: usize, n: Noun) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        // ~/  %hash-noun-varlen
        // |=  n=*
        // ^-  noun-digest
        // =/  leaf=(list @)  (leaf-sequence:shape n)
        let leaf = leaf_sequence_impl::<Belt>(n).unwrap();

        // =/  dyck=(list @)  (dyck:shape n)
        let dyck = dyck(n).unwrap();

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
        let ret = stage.out.len();
        stage.out.resize(ret + DIGEST_LENGTH, Melt(0));

        stage.ops.push(ReduceOp {
            source,
            ty: ReduceTy::Variable(len),
            destination: ret,
        });

        ret
    }

    fn push_list_inner(&mut self, stage: usize, l: &[Hashable]) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage0 = self.stages.get_mut(stage).unwrap();
        let ret = stage0.out.len();
        stage0.out.push(Melt::from_u64(l.len() as _));

        let mut next = ret + 1;
        for h in l {
            let out = self.push(stage, h);
            assert_eq!(out, next);
            next = out + DIGEST_LENGTH;
        }

        let stage = self.stages.get_mut(stage).unwrap();
        // Push dyck shape
        stage.out.push(Melt::zero());
        let shape = [0, 0, 1, 0, 1, 0, 1, 0, 1, 1].map(Melt::from_u64);
        stage
            .out
            .extend(core::iter::repeat_n(shape, l.len()).flatten());

        ret
    }

    fn reduce(mut self) -> Vec<NounDigest> {
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

    fn push(&mut self, stage: usize, h: &Hashable) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }
        match h {
            Hashable::Hash(d) => {
                // ?:  ?=(%hash -.h)
                //   p.h
                let stage = self.stages.get_mut(stage).unwrap();
                let ret = stage.out.len();
                stage.out.extend_from_slice(&d[..]);
                ret
            }
            Hashable::Leaf(n) => {
                // ?:  ?=(%leaf -.h)
                //   (hash-noun-varlen p.h)
                self.push_noun(stage, *n)
            }
            Hashable::List(l) => {
                // ?:  ?=(%list -.h)
                //   (hash-noun-varlen (turn p.h hash-hashable))
                /*let mut v = vec![Melt::from_u64((l.len() * 5 + 1) as _)];
                for e in l {
                    let d = hash_hashable_impl(stack, e)?;
                    v.extend_from_slice(&d);
                }
                v.push(Melt::zero());
                for _ in 0..l.len() {
                    let shape = [0, 0, 1, 0, 1, 0, 1, 0, 1, 1].map(Melt::from_u64);
                    v.extend_from_slice(&shape);
                }
                Ok(hash_varlen(&v))*/
                let source = self.push_list_inner(stage + 1, l);

                let stage = self.stages.get_mut(stage).unwrap();
                let ret = stage.out.len();
                stage.out.resize(ret + DIGEST_LENGTH, Melt(0));

                stage.ops.push(ReduceOp {
                    source,
                    ty: ReduceTy::Variable(2 + (DIGEST_LENGTH + 10) * l.len()),
                    destination: ret,
                });

                ret
            }
            Hashable::Mary(ma) => {
                //   %-  hash-hashable

                //   :-  leaf+step.p.h
                let step = self.push_noun(stage + 1, D(ma.step as _));

                //   :-  leaf+len.array.p.h
                let len = self.push_noun(stage + 2, D(ma.len as _));

                //   hash+(hash-belts-list (bpoly-to-list array:(~(change-step ave p.h) 1)))
                let hash_src =
                    self.push_varlen(stage + 3, ma.dat.iter().copied().map(Melt::from_u64));
                let stage2 = self.stages.get_mut(stage + 2).unwrap();
                let hash = stage2.out.len();
                stage2.out.resize(hash + DIGEST_LENGTH, Melt(0));
                stage2.ops.push(ReduceOp {
                    source: hash_src,
                    ty: ReduceTy::Variable(ma.dat.len()),
                    destination: hash,
                });
                assert_eq!(len + DIGEST_LENGTH, hash);

                let stage1 = self.stages.get_mut(stage + 1).unwrap();
                let arr = stage1.out.len();
                stage1.out.resize(arr + DIGEST_LENGTH, Melt(0));
                stage1.ops.push(ReduceOp {
                    source: len,
                    ty: ReduceTy::Fixed,
                    destination: arr,
                });
                assert_eq!(step + DIGEST_LENGTH, arr);

                let stage = self.stages.get_mut(stage).unwrap();
                let ret = stage.out.len();
                stage.out.resize(ret + DIGEST_LENGTH, Melt(0));
                stage.ops.push(ReduceOp {
                    source: step,
                    ty: ReduceTy::Fixed,
                    destination: ret,
                });

                ret
            }
            Hashable::Pair(a, b) => {
                // %-  hash-ten-cell
                // [$(h p.h) $(h q.h)]
                let a = self.push(stage + 1, a);
                let b = self.push(stage + 1, b);
                assert_eq!(a + DIGEST_LENGTH, b);
                let stage = self.stages.get_mut(stage).unwrap();
                let ret = stage.out.len();
                stage.out.resize(ret + DIGEST_LENGTH, Melt(0));
                stage.ops.push(ReduceOp {
                    source: a,
                    ty: ReduceTy::Fixed,
                    destination: ret,
                });
                ret
            }
        }
    }
}

enum Hashable<'a> {
    // [p=hashable q=hashable]
    Pair(Box<Hashable<'a>>, Box<Hashable<'a>>),
    // [%leaf p=*]
    Leaf(Noun),
    // [%hash p=noun-digest]
    Hash(NounDigest),
    // [%list p=(list hashable)]
    List(Vec<Hashable<'a>>),
    // [%mary p=mary]
    Mary(MarySlice<'a>),
}

impl<'a> Hashable<'a> {
    unsafe fn from_noun(h: Noun) -> core::result::Result<Self, JetErr> {
        let h = h.as_cell()?;
        let ty = h.head().as_direct().map(|v| v.data());

        match ty {
            Ok(tas!(b"hash")) => {
                //trace!("hash");
                // ?:  ?=(%hash -.h)
                //   p.h
                Ok(Self::Hash(
                    h.tail()
                        .uncell()?
                        .map(|v| Belt(v.as_atom().unwrap().as_u64().unwrap()))
                        .map(Melt::from),
                ))
            }
            Ok(tas!(b"leaf")) => {
                // ?:  ?=(%leaf -.h)
                //   (hash-noun-varlen p.h)
                Ok(Self::Leaf(h.tail()))
            }
            Ok(tas!(b"list")) => {
                //trace!("list");
                // ?:  ?=(%list -.h)
                //   (hash-noun-varlen (turn p.h hash-hashable))
                let mut v = vec![];

                for e in HoonList::try_from(h.tail()).ok().into_iter().flatten() {
                    v.push(Self::from_noun(e)?);
                }

                Ok(Self::List(v))
            }
            Ok(tas!(b"mary")) => {
                let Ok(ma) = MarySlice::try_from(h.tail()) else {
                    return jet_err();
                };
                Ok(Self::Mary(ma))
            }
            _ => {
                let left = Self::from_noun(h.head())?;
                let right = Self::from_noun(h.tail())?;
                Ok(Self::Pair(left.into(), right.into()))
            }
        }
    }
}

impl<'a> TryFrom<&'a Noun> for Hashable<'a> {
    type Error = JetErr;

    fn try_from(value: &'a Noun) -> std::result::Result<Self, Self::Error> {
        unsafe { Self::from_noun(*value) }
    }
}

pub fn hash_hashable(stack: &mut NockStack, h: Noun) -> Result {
    let h = Hashable::try_from(&h)?;
    let r = hash_hashable_impl(&h);
    let r = r.map(Belt::from).map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &r))
}

fn hash_hashable_impl(h: &Hashable) -> NounDigest {
    // ~/  %hash-hashable
    // |=  h=hashable
    // ^-  noun-digest
    let mut engine = HashEngine::default();
    engine.push(0, h);
    engine.reduce()[0]
}

/// Inplace modify list to a tuple.
/// If you wish to make this jettable, create a copy of the list, and then call this func.
pub fn list_to_tuple_inplace(n: Noun) -> Result {
    let mut prev = None;
    let mut cell = n.as_cell()?;

    loop {
        if let Ok(tail) = cell.tail().as_cell() {
            prev = Some(cell);
            cell = tail;
        } else {
            if let Some(mut prev) = prev {
                unsafe { (*prev.to_raw_pointer_mut()).tail = cell.head() };
                return Ok(n);
            } else {
                return Ok(cell.head());
            }
        }
    }
}

pub fn dyck(t: Noun) -> core::result::Result<Vec<Belt>, JetErr> {
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
    Ok(res)
}

pub fn bp_build_merk_heap(stack: &mut NockStack, ma: Noun) -> Result {
    build_merk_heap_impl::<Belt>(stack, ma)
}

pub fn build_merk_heap(stack: &mut NockStack, ma: Noun) -> Result {
    build_merk_heap_impl::<Felt>(stack, ma)
}

pub fn build_merk_heap_impl<T: ElementEx>(stack: &mut NockStack, ma: Noun) -> Result {
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
    let Ok(m) = MarySlice::try_from(ma) else {
        return jet_err();
    };

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
        //let hh1 = hash_hashable_impl(stack, &hbp)?;
        //let mut engine = HashEngine::default();
        engine.push(0, &hbp);
        //let hh2 = engine.reduce();
        //assert_eq!(hh2.len(), 1);
        //assert_eq!(hh1, hh2[0]);
        //res_l.push(hh2);
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
    let rl = res.len() * 5;
    let rc = res.capacity() * 5;
    let r = res.as_mut_ptr() as *mut Melt;
    core::mem::forget(res);
    // SAFETY: NounDigest is 5 u64, and it's all contiguous, therefore safe to transmute.
    let res = unsafe { Vec::from_raw_parts(r, rl, rc) };
    let res: BPolyVec = PolyVec(res).into();
    // SAFETY: Same here
    let d = unsafe { core::slice::from_raw_parts(res.0.as_ptr() as *const u64, res.len()) };
    let (res, res_ma) = new_handle_mut_mary(stack, 5, res.len() / 5);
    res_ma.dat.copy_from_slice(d);
    let heap_mary = finalize_mary(stack, res_ma.step as _, res_ma.len as _, res);

    // :-  (xeb len.array.m)          :: compute height of heap
    // :-  %+  snag-as-digest:tip5      :: retrieve the 0th entry of the heap and return it
    //       heap-mary                  ::   as a tip5 hash digest
    //     0
    let digest = snag_as_digest(stack, heap_mary, 0)?;

    // heap-mary
    Ok(T(stack, &[D(height as u64), digest, heap_mary]))
}

fn hashable_poly<'a, T: ElementEx>(p: PolySlice<'a, T>) -> Hashable<'a> {
    let dat =
        unsafe { core::slice::from_raw_parts(p.0.as_ptr() as *const u64, p.0.len() * T::len()) };
    Hashable::Mary(MarySlice {
        step: T::len() as _,
        len: p.len() as _,
        dat,
    })
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
