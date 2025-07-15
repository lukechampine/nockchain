use nockvm::jets::JetErr;
use nockvm::mem::NockStack;
use nockvm::noun::*;
use nockvm_macros::tas;
use std::iter::once;

use zkvm_jetpack::form::mary::MarySlice;
use zkvm_jetpack::form::{BPolyVec, Belt, Element, ElementEx, FPolyVec, Felt, Melt, PolyVec};
use zkvm_jetpack::hand::handle::{finalize_poly, new_handle_mut_slice};
use zkvm_jetpack::hand::structs::HoonList;
use zkvm_jetpack::jets::utils::jet_err;
use zkvm_jetpack::noun::noun_ext::NounExt;

use super::three::{Tip5Tog, absorb_sponge, new_sponge};
use super::hash::{HashEngine, NounDigest};

fn poly_noun<T: Element + Copy>(stack: &mut NockStack, p: PolyVec<T>) -> Noun {
    let (ret, slc) = new_handle_mut_slice(stack, Some(p.0.len()));
    slc.copy_from_slice(&p.0);
    finalize_poly(stack, Some(slc.len()), ret)
}

fn digest_noun(stack: &mut NockStack, v: NounDigest) -> Noun {
    let v = v.map(Belt::from).map(|v| Atom::new(stack, v.0).as_noun());
    T(stack, &v)
}

fn digest(n: Noun) -> core::result::Result<NounDigest, JetErr> {
    Ok(n.uncell()?
        .map(|v| v.as_atom().unwrap().as_u64().unwrap())
        .map(Melt::from_u64))
}

// +$  noun-digests  (list noun-digest:tip5)
// +$  proof-path  [leaf=fpoly path=noun-digests]
#[derive(Clone)]
pub struct ProofPath<T> {
    leaf: PolyVec<T>,
    path: Vec<NounDigest>,
}

impl<T: Element + Copy> ProofPath<T> {
    pub fn to_noun(self, stack: &mut NockStack) -> Noun {
        let leaf = poly_noun(stack, self.leaf);
        let path = self
            .path
            .into_iter()
            .map(|v| digest_noun(stack, v))
            .chain(once(D(0)))
            .collect::<Vec<_>>();
        let path = T(stack, &path);
        T(stack, &[leaf, path])
    }
}

impl<T: Element> TryFrom<Noun> for ProofPath<T> {
    type Error = JetErr;

    fn try_from(value: Noun) -> std::result::Result<Self, Self::Error> {
        let [leaf, path_hl] = value.uncell()?;
        let leaf = PolyVec::try_from(leaf)?;
        let mut path = vec![];
        for p in HoonList::try_from(path_hl).ok().into_iter().flatten() {
            path.push(digest(p)?);
        }
        Ok(Self { leaf, path })
    }
}

type ProofPathFp = ProofPath<Felt>;
type ProofPathBf = ProofPath<Belt>;

// +$  proof-data
#[derive(Clone)]
pub enum ProofData {
    // [%m-root p=noun-digest:tip5]  :: merk-root
    MRoot(NounDigest),
    // [%puzzle commitment=noun-digest:tip5 nonce=noun-digest:tip5 len=@ p=*]
    Puzzle {
        commitment: NounDigest,
        nonce: NounDigest,
        len: u64,
        p: Noun,
    },
    // [%codeword p=fpoly]
    Codeword(FPolyVec),
    // [%terms p=bpoly]  :: terminals
    Terms(BPolyVec),
    // [%m-paths a=proof-path b=proof-path c=proof-path]
    MPaths(ProofPathFp, ProofPathFp, ProofPathFp),
    // [%m-path p=proof-path]   ::  merk-path
    MPath(ProofPathFp),
    // [%m-pathbf p=proof-path-bf]  ::  merk-path-bf
    MPathBf(ProofPathBf),
    // [%comp-m p=noun-digest:tip5 num=@]  ::  composition-merk
    CompM(NounDigest, u64),
    // [%evals p=fpoly]  ::  evaluations
    Evals(FPolyVec),
    // [%heights p=(list @)]  ::  n, where 2^n is the number of rows
    Heights(Noun),
    // [%poly p=bpoly]
    Poly(BPolyVec),
}

impl TryFrom<Noun> for ProofData {
    type Error = JetErr;

    fn try_from(value: Noun) -> std::result::Result<Self, Self::Error> {
        let c = value.as_cell()?;
        let h = c.head().as_direct()?.data();
        let t = c.tail();

        Ok(match h {
            tas!(b"m-root") => Self::MRoot(digest(t)?),
            tas!(b"puzzle") => {
                let [commitment, nonce, len, p] = t.uncell()?;
                let commitment = digest(commitment)?;
                let nonce = digest(nonce)?;
                let len = len.as_atom()?.as_u64()?;

                Self::Puzzle {
                    commitment,
                    nonce,
                    len,
                    p,
                }
            }
            tas!(b"codeword") => Self::Codeword(t.try_into()?),
            tas!(b"terms") => Self::Terms(t.try_into()?),
            tas!(b"m-paths") => {
                let [a, b, c] = t.uncell()?;
                Self::MPaths(a.try_into()?, b.try_into()?, c.try_into()?)
            }
            tas!(b"m-path") => Self::MPath(t.try_into()?),
            tas!(b"m-pathbf") => Self::MPathBf(t.try_into()?),
            tas!(b"comp-m") => {
                let [h, v] = t.uncell()?;
                Self::CompM(digest(h)?, v.as_atom()?.as_u64()?)
            }
            tas!(b"evals") => Self::Evals(t.try_into()?),
            tas!(b"heights") => Self::Heights(t),
            tas!(b"poly") => Self::Poly(t.try_into()?),
            _ => jet_err()?,
        })
    }
}

impl ProofData {
    fn discrim(&self) -> u64 {
        match self {
            Self::MRoot(_) => tas!(b"m-root"),
            Self::Puzzle { .. } => tas!(b"puzzle"),
            Self::Codeword(_) => tas!(b"codeword"),
            Self::Terms(_) => tas!(b"terms"),
            Self::MPaths(_, _, _) => tas!(b"m-paths"),
            Self::MPath(_) => tas!(b"m-path"),
            Self::MPathBf(_) => tas!(b"m-pathbf"),
            Self::CompM(_, _) => tas!(b"comp-m"),
            Self::Evals(_) => tas!(b"evals"),
            Self::Heights(_) => tas!(b"heights"),
            Self::Poly(_) => tas!(b"poly"),
        }
    }

    pub fn to_noun(self, stack: &mut NockStack) -> Noun {
        let discrim = self.discrim();
        let t = match self {
            Self::MRoot(d) => digest_noun(stack, d),
            Self::Puzzle {
                commitment,
                nonce,
                len,
                p,
            } => {
                let commitment = digest_noun(stack, commitment);
                let nonce = digest_noun(stack, nonce);
                T(stack, &[commitment, nonce, D(len), p])
            }
            Self::Codeword(b) => poly_noun(stack, b),
            Self::Terms(b) => poly_noun(stack, b),
            Self::MPaths(a, b, c) => {
                let a = a.to_noun(stack);
                let b = b.to_noun(stack);
                let c = c.to_noun(stack);
                T(stack, &[a, b, c])
            }
            Self::MPath(p) => p.to_noun(stack),
            Self::MPathBf(p) => p.to_noun(stack),
            Self::CompM(d, v) => {
                let d = digest_noun(stack, d);
                let v = Atom::new(stack, v).as_noun();
                T(stack, &[d, v])
            }
            Self::Evals(p) => poly_noun(stack, p),
            Self::Heights(h) => h,
            Self::Poly(p) => poly_noun(stack, p),
        };
        T(stack, &[D(discrim), t])
    }

    // ++  hashable-proof-data
    #[inline(never)]
    fn hash_proof_data(self, engine: &mut HashEngine) -> usize {
        //   ~/  %hashable-proof-data
        //   |=  pd=proof-data
        //   ^-  hashable:tip5
        //   ?-    -.pd
        engine.ensure_stages(1);
        let a = engine.push_noun(1, D(self.discrim())).unwrap();

        fn as_mary<T: ElementEx>(s: &[T]) -> MarySlice {
            MarySlice {
                len: s.len() as _,
                step: T::len() as _,
                dat: unsafe {
                    core::slice::from_raw_parts(s.as_ptr() as *const u64, (s.len() * T::len()) as _)
                },
            }
        }

        let b = match self {
            Self::MRoot(r) => {
                // %m-root    [leaf+%m-root hash+p.pd]
                engine.push_hash(1, r)
            }
            // %puzzle    [leaf+%puzzle hash+commitment.pd hash+nonce.pd leaf+len.pd leaf+p.pd]
            Self::Puzzle {
                commitment,
                nonce,
                len,
                p,
            } => {
                engine.ensure_stages(4);

                let commitment = engine.push_hash(2, commitment);
                let nonce = engine.push_hash(3, nonce);
                let len = engine.push_noun(4, D(len)).unwrap();
                let p = engine.push_noun(4, p).unwrap();

                let res = engine.push_pair(3, len, p);
                let res = engine.push_pair(2, nonce, res);
                engine.push_pair(1, commitment, res)
            }
            // %comp-m    [leaf+%comp-m hash+p.pd leaf+num.pd]
            Self::CompM(p, num) => {
                engine.ensure_stages(4);

                let p = engine.push_hash(2, p);
                let num = engine.push_noun(2, D(num)).unwrap();

                engine.push_pair(1, p, num)
            }
            // %heights   [leaf+%heights leaf+p.pd]
            Self::Heights(p) => engine.push_noun(1, p).unwrap(),
            // %codeword  [leaf+%codeword (hashable-fpoly:tip5 p.pd)]
            Self::Codeword(p) => {
                let ma = as_mary(&p.0);
                engine.push_mary(1, ma)
            }
            // %evals     [leaf+%evals (hashable-fpoly:tip5 p.pd)]
            Self::Evals(p) => {
                let ma = as_mary(&p.0);
                engine.push_mary(1, ma)
            }
            // %terms     [leaf+%terms (hashable-bpoly:tip5 p.pd)]
            Self::Terms(p) => {
                let ma = as_mary(&p.0);
                engine.push_mary(1, ma)
            }
            // %poly      [leaf+%poly (hashable-bpoly:tip5 p.pd)]
            Self::Poly(p) => {
                let ma = as_mary(&p.0);
                engine.push_mary(1, ma)
            }
            //:
            //   %m-pathbf
            // :-  leaf+%m-pathbf
            // :-  (hashable-bpoly:tip5 leaf.p.pd)
            // (hashable-noun-digests:tip5 path.p.pd)
            Self::MPathBf(p) => {
                engine.ensure_stages(2);

                let ma = as_mary(&p.leaf.0);
                let ma = engine.push_mary(2, ma);
                let li = engine.push_list(2, p.path.into_iter()).unwrap();

                engine.push_pair(1, ma, li)
            }
            //:
            //   %m-path
            // :-  leaf+%m-mpath
            // :-  (hashable-fpoly:tip5 leaf.p.pd)
            // (hashable-noun-digests:tip5 path.p.pd)
            Self::MPath(p) => {
                engine.ensure_stages(2);

                let ma = as_mary(&p.leaf.0);
                let ma = engine.push_mary(2, ma);
                let li = engine.push_list(2, p.path.into_iter()).unwrap();

                engine.push_pair(1, ma, li)
            }
            //:
            //   %m-paths
            // :-  leaf+%m-mpaths
            // :+  :-  (hashable-fpoly:tip5 leaf.a.pd)
            //     (hashable-noun-digests:tip5 path.a.pd)
            //   :-  (hashable-fpoly:tip5 leaf.b.pd)
            //   (hashable-noun-digests:tip5 path.b.pd)
            // :-  (hashable-fpoly:tip5 leaf.c.pd)
            // (hashable-noun-digests:tip5 path.c.pd)
            Self::MPaths(a, b, c) => {
                let ma = as_mary(&a.leaf.0);
                let ma = engine.push_mary(3, ma);
                let li = engine.push_list(3, a.path.into_iter()).unwrap();
                let a = engine.push_pair(2, ma, li);

                let ma = as_mary(&b.leaf.0);
                let ma = engine.push_mary(4, ma);
                let li = engine.push_list(4, b.path.into_iter()).unwrap();
                let b = engine.push_pair(3, ma, li);

                let ma = as_mary(&c.leaf.0);
                let ma = engine.push_mary(4, ma);
                let li = engine.push_list(4, c.path.into_iter()).unwrap();
                let c = engine.push_pair(3, ma, li);

                let bc = engine.push_pair(2, b, c);
                engine.push_pair(1, a, bc)
            }
        };
        engine.push_pair(0, a, b)
    }
}

/*pub fn absorb_proof_objects(stack: &mut NockStack, sam: Noun) -> Result {
    // ~/  %absorb-proof-objects
    // |=  [objs=proof-objects hashes=(list noun-digest:tip5)]
    let [objs, hashes] = sam.uncell()?;
    let objs = HoonList::try_from(objs)
        .ok()
        .into_iter()
        .flatten()
        .map(|v| ProofData::try_from(v).unwrap())
        .collect::<Vec<_>>();
    let hashes = HoonList::try_from(hashes)
        .ok()
        .into_iter()
        .flatten()
        .map(|v| {
            let v: [Noun; 5] = v.uncell().unwrap();
            v.map(|v| v.as_atom().unwrap().as_u64().unwrap())
                .map(Melt::from_u64)
        })
        .collect::<Vec<_>>();
    // ^+  tog:tip5
    let tog = absorb_proof_objects_impl(&objs, &hashes);
    let r = tog.to_noun(stack);
    Ok(r)
}*/

// $%  $:  version=%1
//         objects=proof-objects
//         hashes=(list noun-digest:tip5)
//         read-index=@
//     ==
//   ::
//     $:  version=%0
//         objects=proof-objects
//         hashes=(list noun-digest:tip5)
//         read-index=@
//     ==
// ==
pub struct Proof {
    version: usize,
    pub objects: Vec<ProofData>,
    pub hashes: Vec<NounDigest>,
    pub read_index: u64,
}

impl Proof {
    pub fn push(&mut self, obj: ProofData) {
        // ~/  %push
        // |=  dat=proof-data
        // ^-  proof
        // :^    %0
        //     (snoc objects dat)
        //   (snoc hashes (hash-hashable:tip5 (hashable-proof-data dat)))
        // read-index
        let mut engine = HashEngine::default();
        let i = obj.clone().hash_proof_data(&mut engine);
        self.hashes.push(engine.reduce()[i]);
        self.objects.push(obj);
        self.version = 0;
    }

    pub fn to_noun(self, stack: &mut NockStack) -> Noun {
        let o = self
            .objects
            .iter()
            .map(|v| v.clone().to_noun(stack))
            .chain(once(D(0)))
            .collect::<Vec<_>>();
        let o = T(stack, &o);

        let h = self
            .hashes
            .iter()
            .map(|v| {
                let v = v.map(|v| Atom::new(stack, Belt::from(v).0).as_noun());
                T(stack, &v)
            })
            .chain(once(D(0)))
            .collect::<Vec<_>>();
        let h = T(stack, &h);

        let r = Atom::new(stack, self.read_index).as_noun();

        T(stack, &[D(self.version as _), o, h, r])
    }
}

impl TryFrom<Noun> for Proof {
    type Error = JetErr;

    fn try_from(value: Noun) -> std::result::Result<Self, Self::Error> {
        let [v, objects_in, hashes_in, read_index] = value.uncell()?;

        let mut objects = vec![];
        for o in HoonList::try_from(objects_in).ok().into_iter().flatten() {
            objects.push(ProofData::try_from(o)?);
        }

        let mut hashes = vec![];
        for h in HoonList::try_from(hashes_in).ok().into_iter().flatten() {
            let h: [Noun; 5] = h.uncell()?;
            hashes.push(
                h.map(|v| v.as_atom().unwrap().as_u64().unwrap())
                    .map(Melt::from_u64),
            );
        }

        let read_index = read_index.as_atom()?.as_u64()?;

        let version = v.as_direct()?.data() as usize;
        assert!(version <= 1);

        Ok(Self {
            version,
            objects,
            hashes,
            read_index,
        })
    }
}

pub fn absorb_proof_objects_impl(objs: &[ProofData], hashes: &[NounDigest]) -> Tip5Tog {
    // =.  objs  (slag (lent hashes) objs)
    let (_, objs) = objs.split_at(core::cmp::min(hashes.len(), objs.len()));
    // =/  lis-digests=[%list (list hashable:tip5)]
    //   =/  h  (hashable-noun-digests:tip5 hashes)
    //   ?>  ?=(%list -.h)
    //   h
    // =/  lis-objects=[%list (list hashable:tip5)]
    //   =/  h  (hashable-proof-objects objs)
    //   ?>  ?=(%list -.h)
    //   h
    let mut engine = HashEngine::default();
    for h in objs {
        h.clone().hash_proof_data(&mut engine);
    }
    let lis_objects = engine.reduce();
    // =/  big-lis=(list noun-digest:tip5)
    //   (turn `(list hashable:tip5)`(weld +.lis-digests +.lis-objects) hash-hashable:tip5)
    let big_lis = hashes.iter().copied().chain(lis_objects);
    // =/  sponge  (new:sponge:tip5)
    let mut sponge = new_sponge(true);
    // |-
    for d in big_lis {
        // ?~  big-lis
        //   (new:tog:tip5 sponge:sponge)
        // =+  [a=@ b=@ c=@ d=@ e=@]=i.big-lis
        // =/  lis=(list belt)  [a b c d e ~]
        // $(big-lis t.big-lis, sponge (absorb:sponge lis))
        absorb_sponge::<true, _>(&mut sponge, &d);
    }

    Tip5Tog { sponge }
}
