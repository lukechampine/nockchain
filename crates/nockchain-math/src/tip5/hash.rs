use nockapp::Noun;
use nockvm::jets::list::util::{lent, weld};
use nockvm::jets::JetErr;
use nockvm::noun::{NounAllocator, D, T};
use noun_serde::{NounDecode, NounEncode};

use super::*;
use crate::based;
use crate::belt::Belt;
use crate::melt::Melt;
use crate::shape::*;

// assert that input is made of base field elements
pub fn assert_all_based(vecbelt: &[Belt]) {
    vecbelt.iter().for_each(|b| based!(b.0));
}

// calc q and r for vecbelt, based on RATE
pub fn tip5_calc_q_r(input_vec: &Vec<Belt>) -> (usize, usize) {
    let lent_input = input_vec.len();
    let (q, r) = (lent_input / RATE, lent_input % RATE);
    (q, r)
}

// pad vecbelt with ~[1 0 ... 0] to be a multiple of rate
pub fn tip5_pad_vecbelt(input_vec: &mut Vec<Belt>, r: usize) {
    input_vec.push(Belt(1));
    for _i in 0..(RATE - r) - 1 {
        input_vec.push(Belt(0));
    }
}

// monitify vecbelt (bring into montgomery space)
pub fn tip5_montify_vecbelt(mut input_vec: Vec<Belt>) -> Vec<Melt> {
    input_vec
        .iter_mut()
        .for_each(|v| *v = Belt(Melt::from(*v).0));
    unsafe { core::mem::transmute(input_vec) }
}

// calc digest
pub fn tip5_calc_digest(sponge: &[Melt; 16]) -> [Belt; 5] {
    let mut digest = [Belt(0); DIGEST_LENGTH];
    for i in 0..DIGEST_LENGTH {
        digest[i] = sponge[i].into();
    }
    digest
}

// absorb complete input
pub fn tip5_absorb_input(input_vec: &mut Vec<Melt>, sponge: &mut [Melt; 16], q: usize) {
    let mut cnt_q = q;
    let mut input_to_absorb = input_vec.as_slice();
    loop {
        let (scag_input, slag_input) = input_to_absorb.split_at(RATE);
        tip5_absorb_rate(sponge, scag_input);

        if cnt_q == 0 {
            break;
        }
        cnt_q -= 1;
        input_to_absorb = slag_input;
    }
}

// absorb one part of input (size RATE)
pub fn tip5_absorb_rate(sponge: &mut [Melt; 16], input: &[Melt]) {
    assert_eq!(input.len(), RATE);

    for copy_pos in 0..RATE {
        sponge[copy_pos] = input[copy_pos];
    }

    permute(sponge);
}

pub fn hash_varlen(mut input_vec: Vec<Belt>) -> [Belt; 5] {
    let mut sponge = create_init_sponge_variable();

    // assert that input is made of base field elements
    assert_all_based(&input_vec);

    // pad input with ~[1 0 ... 0] to be a multiple of rate
    let (q, r) = tip5_calc_q_r(&input_vec);
    tip5_pad_vecbelt(&mut input_vec, r);

    // bring input into montgomery space
    let mut input_vec = tip5_montify_vecbelt(input_vec);

    // process input in batches of size RATE
    tip5_absorb_input(&mut input_vec, &mut sponge, q);

    // calc digest

    tip5_calc_digest(&sponge)
}

pub fn create_init_sponge_variable() -> [Melt; STATE_SIZE] {
    [Melt(0u64); STATE_SIZE]
}
pub fn create_init_sponge_fixed() -> [Melt; STATE_SIZE] {
    [
        0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 4294967295u64, 4294967295u64,
        4294967295u64, 4294967295u64, 4294967295u64, 4294967295u64,
    ]
    .map(Melt)
}

pub fn hash_10(input: [Belt; 10]) -> [Belt; 5] {
    // check input
    assert_all_based(&input);

    // bring input into montgomery space
    let input = input.map(Melt::from);

    // create init sponge (%fixed)
    let mut sponge = create_init_sponge_fixed();

    // process input (q=1, so one batch only)
    //tip5_absorb_input(&mut input_vec, &mut sponge, q);
    tip5_absorb_rate(&mut sponge, &input);

    //  calc digest
    tip5_calc_digest(&sponge)
}

pub fn hash_noun_varlen<A: NounAllocator>(stack: &mut A, n: Noun) -> Result<Noun, JetErr> {
    let leaf = leaf_sequence(stack, n)?;
    let dyck = dyck(stack, n)?;
    let size = lent(leaf).map(|x| D(x as u64))?;

    // [size (weld leaf dyck)]
    let weld = weld(stack, leaf, dyck)?;
    let arg = T(stack, &[size, weld]);

    hash_belts_list(stack, arg)
}

pub fn hash_noun_varlen_digest<A: NounAllocator>(
    stack: &mut A,
    n: Noun,
) -> Result<[Belt; 5], JetErr> {
    let noun_res = hash_noun_varlen(stack, n)?;
    let digest = <[Belt; 5]>::from_noun(&noun_res)?;
    Ok(digest)
}

pub fn hash_belts_list<A: NounAllocator>(alloc: &mut A, input: Noun) -> Result<Noun, JetErr> {
    let input_vec = <Vec<Belt>>::from_noun(&input)?;
    let digest = hash_varlen(input_vec);
    let res = digest.to_noun(alloc);
    Ok(res)
}
