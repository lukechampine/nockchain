use crate::form::mary::MarySlice;
use crate::form::math::tip5::{self, CAPACITY, DIGEST_LENGTH, RATE, STATE_SIZE};
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

pub fn init_tip5_state(domain: DirectAtom) -> core::result::Result<[u64; STATE_SIZE], JetErr> {
    match domain.data() {
        // ^~((reap state-size 0))
        tas!(b"variable") => Ok([0; STATE_SIZE]),
        // ^~((weld (reap rate 0) (reap capacity (montify 1))))
        tas!(b"fixed") => {
            let zero = [0; RATE];
            let mont = [montify(1); CAPACITY];
            Ok(concat_arrays!(zero, mont))
        }
        _ => Err(BAIL_EXIT),
    }
}

pub fn hash_10(input: [Belt; 10]) -> core::result::Result<NounDigest, JetErr> {
    // ::  +hash-10: hash list of 10 belts into a list of 5 belts
    // |=  input=(list belt)
    // ::  output length is 5
    // ^-  (list belt)

    // Verify that this list has length 10 and all elems are direct:
    // ?>  =((lent input) rate)
    // ?>  (levy input based)
    // FIXME: acc verify this

    // =.  input   (turn input montify)
    let input = input.map(|v| montify(v.0));

    // =/  sponge  (init-tip5-state %fixed)
    let mut sponge = init_tip5_state(DirectAtom::new(tas!(b"fixed"))?)?;

    // =.  sponge  (permutation (weld input (slag rate sponge)))
    sponge[..RATE].copy_from_slice(&input);
    tip5::permute(&mut sponge);

    // (turn (scag digest-length sponge) mont-reduction)
    let ret: [u64; DIGEST_LENGTH] = sponge[..DIGEST_LENGTH].try_into().unwrap();

    Ok(ret.map(|v| Belt(mont_reduction(v as _))))
}

pub fn hash_belts_list(belts: &[Belt]) -> core::result::Result<NounDigest, JetErr> {
    // |=  belts=(list belt)
    // ^-  noun-digest:tip5
    // =-  ?>  ?=(noun-digest -)  -
    // %-  list-to-tuple
    // (hash-varlen belts)
    let hashed = hash_varlen(belts)?;
    Ok(hashed)
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

    hash_belts_list(&belts)
}

pub fn new_sponge() -> core::result::Result<[u64; tip5::STATE_SIZE], JetErr> {
    init_tip5_state(DirectAtom::new(tas!(b"variable"))?)
}

pub fn absorb_sponge(
    sponge: &mut [u64; tip5::STATE_SIZE],
    input: &[Belt],
) -> core::result::Result<(), JetErr> {
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
    let v = RATE - r - 1;
    let mut input = input.to_vec();
    input.push(Belt(1));
    input.resize(input.len() + v, Belt(0));

    // ::  bring input into montgomery space
    // =.  input  (turn input montify)
    let mut input = input.into_iter().map(|v| montify(v.0));

    // |-
    // ?:  =(q 0)
    //   rng
    for _ in (0..=q).rev() {
        // =.  sponge  (absorb-rate (scag rate input))

        // ++  absorb-rate
        //   ?>  =((lent input) rate)
        let input_head = [(); RATE].map(|_| input.next().unwrap());

        //   =.  sponge  (weld input (slag rate sponge))
        sponge[..RATE].copy_from_slice(&input_head);
        //   $:permute
        tip5::permute(sponge);
    }

    Ok(())
}

pub fn squeeze_sponge(spo: &mut [u64; tip5::STATE_SIZE]) -> [Belt; RATE] {
    // |.  ^+  [*(list belt) +.$]
    // =*  rng  +.$
    // ::  squeeze out the full rate and bring out of montgomery space
    // =/  output  (turn (scag rate sponge) mont-reduction)
    let ret = <[u64; RATE]>::try_from(&spo[..RATE]).unwrap();
    let ret = ret.map(|v| Belt(mont_reduction(v as _)));

    tip5::permute(spo);

    ret
}

pub fn hash_varlen(input: &[Belt]) -> core::result::Result<NounDigest, JetErr> {
    // |=  input=(list belt)
    // ^-  (list belt)
    // =/  spo  (new:sponge)
    let mut spo = new_sponge()?;

    // =.  spo  (absorb:spo input)
    absorb_sponge(&mut spo, input).inspect_err(|e| println!("1: {e:?}"))?;

    // =^  output  spo
    //   (squeeze:spo)
    let output = squeeze_sponge(&mut spo);

    // (scag digest-length output)
    Ok(output[..DIGEST_LENGTH].try_into().unwrap())
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
        let hashed = hash_10(welded)
            .inspect_err(|e| println!("hash_10 failed: {e:?}"))
            .map_err(|_| JetErr::Punt)?;
        ret.push(hashed);
    }

    Ok(ret)
}

type NounDigest = [Belt; 5];

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
                        .map(|v| Belt(v.as_atom().unwrap().as_u64().unwrap())),
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

                for e in HoonList::try_from(h.tail())? {
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
    let r = hash_hashable_impl(stack, &h)?;
    let r = r.map(|v| Atom::new(stack, v.0).as_noun());
    Ok(T(stack, &r))
}

fn hash_hashable_impl(
    stack: &mut NockStack,
    h: &Hashable,
) -> core::result::Result<NounDigest, JetErr> {
    // ~/  %hash-hashable
    // |=  h=hashable
    // ^-  noun-digest

    match h {
        Hashable::Hash(d) => {
            // ?:  ?=(%hash -.h)
            //   p.h
            Ok(*d)
        }
        Hashable::Leaf(n) => {
            // ?:  ?=(%leaf -.h)
            //   (hash-noun-varlen p.h)
            hash_noun_varlen(*n)
        }
        Hashable::List(l) => {
            // ?:  ?=(%list -.h)
            //   (hash-noun-varlen (turn p.h hash-hashable))
            let mut v = vec![];
            for e in l {
                let d = hash_hashable_impl(stack, e)?;
                let d = d.map(|v| Atom::new(stack, v.0).as_noun());
                let c = T(stack, &d);
                v.push(c);
            }
            v.push(D(0));
            let v = T(stack, &v);
            hash_noun_varlen(v)
        }
        Hashable::Mary(ma) => {
            //   %-  hash-hashable

            //   :-  leaf+step.p.h
            let step = Hashable::Leaf(D(ma.step as _));

            //   :-  leaf+len.array.p.h
            let len = Hashable::Leaf(D(ma.len as _));

            //   hash+(hash-belts-list (bpoly-to-list array:(~(change-step ave p.h) 1)))
            let dat = &ma.dat;
            let dat = unsafe { core::mem::transmute::<&[u64], &[Belt]>(dat) };
            let hash = hash_belts_list(dat).inspect_err(|e| trace!("hbl {e:?}"))?;
            let hash = Hashable::Hash(hash);

            let f = Hashable::Pair(len.into(), hash.into());
            let f = Hashable::Pair(step.into(), f.into());

            hash_hashable_impl(stack, &f)
        }
        Hashable::Pair(a, b) => {
            // %-  hash-ten-cell
            // [$(h p.h) $(h q.h)]
            let p = hash_hashable_impl(stack, a)?;
            let q = hash_hashable_impl(stack, b)?;
            let b = concat_arrays!(p, q);
            hash_10(b)
        }
    }
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
    let mut res_l = Vec::with_capacity(m.len as usize);
    for i in 0..m.len {
        let t = snag_as_bpoly_mary(m, i as usize);
        let hbp = hashable_bpoly(stack, &t);
        let hh = hash_hashable(stack, hbp)?;
        let leaf = leaf_sequence_impl::<Belt>(hh)?;
        let leaf = leaf.try_into().unwrap();
        res_l.push(leaf);
    }

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
    // SAFETY: NounDigest is 5 u64, and it's all contiguous, therefore safe to transmute.
    let d = unsafe { core::slice::from_raw_parts(res.as_ptr() as *const u64, res.len() * 5) };
    let (res, res_ma) = new_handle_mut_mary(stack, 5, res.len());
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

fn hashable_bpoly(stack: &mut NockStack, bp: &BPolySlice) -> Noun {
    let (ret, handle) = new_handle_mut_slice(stack, Some(bp.len()));
    handle.copy_from_slice(&bp.0);
    let ret = finalize_poly(stack, Some(bp.len()), ret);

    T(stack, &[D(tas!(b"mary")), D(1), ret])
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
