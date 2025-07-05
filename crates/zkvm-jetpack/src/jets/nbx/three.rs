use std::iter::once;
use std::mem::MaybeUninit;

use array_concat::concat_arrays;
use nockvm::interpreter::Context;
use nockvm::jets::util::{slot, BAIL_EXIT};
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, Noun, D, T};

use super::one::*;
use super::utils::*;
use super::hash::{NounDigest, HashEngine, leaf_sequence_impl};
use crate::form::mary::{Mary, MarySlice};
use crate::form::math::tip5::{self, CAPACITY, DIGEST_LENGTH, RATE, STATE_SIZE};
use crate::form::poly::Poly;
use crate::form::tip5::permute;
use crate::form::{
    BPolyVec, Belt, Element, ElementEx, Felt,
    Melt, PolySlice, PolyVec,
};
use crate::hand::handle::{finalize_mary, new_handle_mut_mary};
use crate::hand::structs::HoonList;
use crate::jets::utils::jet_err;
use crate::noun::noun_ext::NounExt;

pub fn leaf_sequence(stack: &mut NockStack, t: Noun) -> Result {
    let mut r = leaf_sequence_impl(t)?;
    r.push(D(0));
    Ok(T(stack, &r))
}

pub fn new_sponge(variable: bool) -> [Melt; tip5::STATE_SIZE] {
    if variable {
        // ^~((reap state-size 0))
        [Melt(0); STATE_SIZE]
    } else {
        // ^~((weld (reap rate 0) (reap capacity (montify 1))))
        let zero = [Melt(0); RATE];
        let mont = [Melt::one(); CAPACITY];
        concat_arrays!(zero, mont)
    }
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
            debug_assert_eq!(r, 0);
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

pub fn hash_hashable(stack: &mut NockStack, h: Noun) -> Result {
    let mut engine = HashEngine::default();
    engine.push(0, h)?;
    let r = engine.reduce()[0];
    let r = r.map(Belt::from).map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &r))
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
    let mut engine = HashEngine::default();

    engine.ensure_stages(height - 1);
    engine.push_pair(0, 0, DIGEST_LENGTH);
    for l in 1..(height - 1) {
        for i in (0..(1 << l)).step_by(2) {
            engine.reserve_pair(l);
            for i in i..=(i + 1) {
                engine.push_pair(l, i * 2 * DIGEST_LENGTH, (i * 2 + 1) * DIGEST_LENGTH);
            }
        }
    }
    engine.push_hashes(height - 1, &res_l);

    let mut res = engine
        .reduce_with_intermediates()
        .rev()
        .flatten()
        .collect::<Vec<_>>();

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
