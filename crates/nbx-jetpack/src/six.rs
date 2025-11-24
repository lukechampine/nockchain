use std::iter::once;

use nbx_tip5::base::binv;
use nockchain_math::belt::Belt;
use nockchain_math::felt::Felt;
use nockchain_math::handle::{finalize_mary, new_handle_mut_mary};
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::poly::*;
use nockchain_math::poly_ext::*;
use nockvm::interpreter::Context;
use nockvm::jets::util::BAIL_FAIL;
use nockvm::jets::{JetErr, Result};
use nockvm::mem::NockStack;
use nockvm::noun::{Atom, Noun, Slots, D, T};
use noun_serde::NounEncode;
use num_traits::Pow;
use zkvm_jetpack::form::mary::Mary;
use zkvm_jetpack::form::poly::Poly;

use super::four::{absorb_proof_objects_impl, Proof, ProofData};
use super::one::*;
use super::three::{build_merk_heap_impl, build_merk_proof, MerkHeap};
use crate::four::ProofPath;

// $:  offset=belt
//     omega=belt
//     init-domain-len=@
//     expansion-fac=@
//     num-spot-checks=@
//     folding-deg=@
// ==
#[derive(Clone, Copy)]
pub struct FriInput {
    offset: Belt,
    omega: Belt,
    init_domain_len: u64,
    expansion_fac: u64,
    num_spot_checks: u64,
    folding_deg: u64,
}

impl FriInput {
    pub fn new(max_height: u64) -> Self {
        let expansion_fac = 1 << 6;
        let init_domain_len = max_height.next_power_of_two() * expansion_fac;
        Self {
            offset: Belt(7),
            omega: Belt(init_domain_len).ordered_root().unwrap(),
            init_domain_len,
            expansion_fac,
            num_spot_checks: 50 / 6,
            folding_deg: 8,
        }
    }

    const fn num_rounds(self) -> u64 {
        // ^-  @
        // =/  len  init-domain-len
        let mut len = self.init_domain_len;
        // =/  num  0
        let mut num = 0;
        // |-
        // ?:  &((gth len expansion-fac) (lth (mul 4 num-spot-checks) len))
        while len > self.expansion_fac && (4 * self.num_spot_checks) < len {
            //   $(num +(num), len (div len folding-deg))
            num += 1;
            len /= self.folding_deg;
        }
        // (max 1 (dec num))
        if num > 1 {
            num - 1
        } else {
            1
        }
    }
}

impl TryFrom<Noun> for FriInput {
    type Error = JetErr;

    fn try_from(value: Noun) -> std::result::Result<Self, Self::Error> {
        let [offset, omega, init_domain_len, expansion_fac, num_spot_checks, folding_deg] = value
            .uncell()?
            .map(|v| v.as_atom().unwrap().as_u64().unwrap());
        let offset = Belt(offset);
        let omega = Belt(omega);

        Ok(Self {
            offset,
            omega,
            init_domain_len,
            expansion_fac,
            num_spot_checks,
            folding_deg,
        })
    }
}

pub fn prove_fri_door(context: &mut Context, subj: Noun) -> Result {
    let stack = &mut context.stack;

    let parent_core = subj.slot(7)?;
    let fri = parent_core.slot(6)?;
    let fri = FriInput::try_from(fri)?;

    let sam = subj.slot(6)?;
    let [codeword, stream] = sam.uncell()?;
    let codeword = FPolySlice::try_from(codeword)?;
    let stream = Proof::try_from(stream)?;

    let (codewords, stream) = prove_commit_impl(fri, codeword, stream)?;
    let (fri_indices, stream) = prove_query_impl(stack, fri, codewords, stream)?;
    let fri_indices = T(
        stack,
        &fri_indices
            .into_iter()
            .map(|v| D(v as u64))
            .chain(once(D(0)))
            .collect::<Vec<_>>(),
    );
    let stream = stream.to_noun(stack);
    Ok(T(stack, &[fri_indices, stream]))
}

// +$  codeword-data  [codeword=mary merk=(unit [depth=@ heap=merk-heap])]
pub struct CodewordData {
    codeword: Mary,
    merk: Option<(usize, MerkHeap)>,
}

pub fn prove_query_impl(
    stack: &mut NockStack,
    fri: FriInput,
    codewords: Vec<CodewordData>,
    mut stream: Proof,
) -> core::result::Result<(Vec<usize>, Proof), JetErr> {
    let last_codeword_len = fri.init_domain_len / (fri.folding_deg.pow(fri.num_rounds() as u32));

    let fri_indices = absorb_proof_objects_impl(&stream.objects, &stream.hashes).indices(
        fri.num_spot_checks as usize, fri.init_domain_len as usize, last_codeword_len as usize,
    );

    let mut round_indices = fri_indices.clone();
    for data in &codewords {
        let len = ((data.codeword.step as usize * data.codeword.len as usize) / 3);
        let idx_mod = len / fri.folding_deg as usize;
        for idx in &mut round_indices {
            *idx %= idx_mod;
            let merk = data.merk.as_ref().ok_or(BAIL_FAIL)?;
            let axis = (1 << (merk.0 - 1)) + *idx;
            let leaf = snag_as_poly_mary(data.codeword.as_slice(), *idx as usize);
            let mpath = {
                // TODO: cleaner if we have build_merk_proof_impl that returns Vec<[Melt; 5]>
                let leaf = PolyVec::<Felt>(leaf.0.to_vec()).to_noun(stack);
                let m = merk.1.m.to_noun(stack);
                let opening = build_merk_proof(stack, m, axis as u64)?;
                ProofPath::try_from(T(stack, &[leaf, opening]))?
            };
            stream.push(ProofData::MPath(mpath));
        }
    }

    Ok((fri_indices, stream))
}

pub fn prove_commit_impl(
    fri: FriInput,
    codeword: FPolySlice,
    mut stream: Proof,
) -> core::result::Result<(Vec<CodewordData>, Proof), JetErr> {
    // =-  [(flop codewords) stream]
    // %+  roll  (range +(num-rounds))
    // |=  [round=@ codeword=_codeword codewords=(list codeword-data) omega=_(lift omega) round-offset=_(lift offset) stream=_stream]
    let mut codeword: FPolyVec = PolyVec(codeword.0.into());
    let mut codewords: Vec<CodewordData> = vec![];

    // Keep track of omega's inverse instead of omega as multiplication is faster than division
    let mut omega_inv = Felt::one() / Felt::lift(fri.omega);

    // Determine the twiddles as all p_ntt calls have the same size.
    let inv_len = Felt::from_u64(binv(fri.folding_deg as _));
    let or = Belt(fri.folding_deg as _).ordered_root()?;
    let root = Felt::from_u64(binv(or.0));
    let twiddles = p_ntt_twiddles(fri.folding_deg as _, &root);

    let mut round_offset = Felt::lift(fri.offset);
    for round in 0..fri.num_rounds() {
        // =/  num  (div len.codeword folding-deg)
        let num = codeword.len() / fri.folding_deg as usize;
        // ::
        // ::  sort codeword into cosets
        // =/  cosets=mary
        let mut cosets = Mary {
            step: 3 * (fri.folding_deg as u32),
            len: num as u32,
            dat: Vec::with_capacity(num * fri.folding_deg as usize * 3),
        };
        //   %-  zing-fpolys
        //   %+  turn  (range num)
        //   |=  k=@
        for k in 0..num {
            //   %-  init-fpoly
            //   %+  turn  (range folding-deg)
            //   |=  i=@
            for i in 0..fri.folding_deg {
                //   =/  idx  (add (mul i num) k)
                let idx = (i as usize) * num + k;
                //   (~(snag fop codeword) idx)
                cosets.dat.extend(codeword.0[idx].0.iter().map(|v| v.0));
            }
        }
        // ::
        // ::  send codeword (as cosets) to verifier
        // =/  merk=(pair @ merk-heap:merkle)
        //   (build-merk-heap:merkle cosets)
        let merk = build_merk_heap_impl::<Felt>((&cosets).into())?;
        // =.  stream
        //   (~(push proof-stream stream) [%m-root h.q.merk])
        stream.push(ProofData::MRoot(merk.1.h));
        // ::
        // ::  get challenge from verifier
        // =/  rng  ~(prover-fiat-shamir proof-stream stream)
        let mut rng = absorb_proof_objects_impl(&stream.objects, &stream.hashes);
        // =^  alpha=felt  rng  $:felt:rng
        let alpha = rng.felt();
        // ::
        // ::  compute new codeword
        // =/  new-codeword=fpoly
        //   %-  init-fpoly
        //   %+  turn  (range len.array.cosets)
        //   |=  i=@
        codeword.0.truncate(cosets.len as usize);

        //   =/  eval-point=felt  (fdiv alpha (fmul round-offset (fpow omega i)))
        let mut eval_point = alpha / round_offset;

        for i in 0..(cosets.len as usize) {
            //   =/  coset=fpoly  (~(snag-as-fpoly ave cosets) i)
            let coset = snag_as_poly_mary((&cosets).into(), i);
            //   ::=/  eval-point=felt  (fdiv alpha (fpow omega i))
            //   (fpeval (fp-ifft coset) eval-point)
            let mut icoset = coset.0.to_vec();
            p_ntt_twiddled_inplace(&mut icoset, &twiddles, None);
            pscal_inplace(inv_len, &mut icoset);
            let evaled = peval(PolySlice(&icoset), eval_point);
            codeword.0[i] = evaled;

            eval_point *= omega_inv;
        }
        // ::
        // :*  new-codeword
        //     [[cosets (some merk)] codewords]
        //     (fpow omega folding-deg)
        //     (fpow round-offset folding-deg)
        //     stream
        // ==
        codewords.push(CodewordData {
            codeword: cosets,
            merk: Some(merk),
        });
        omega_inv = omega_inv.pow(fri.folding_deg as usize);
        round_offset = round_offset.pow(fri.folding_deg as usize);
    }

    // NOTE: lifted out from the roll
    // ?:  =(round num-rounds)
    //   ::  If it's the last round, send the raw codeword to the verifier instead
    //   ::  of a merkle tree
    //     :*   zero-fpoly
    //          codewords
    //          (fpow omega folding-deg)
    //          (fpow round-offset folding-deg)
    //          (~(push proof-stream stream) [%codeword codeword])
    //     ==
    // ::
    stream.push(ProofData::Codeword(codeword));

    Ok((codewords, stream))
}
