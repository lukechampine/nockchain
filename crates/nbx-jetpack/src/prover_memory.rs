use array_concat::concat_arrays;
use either::Either;
use nockchain_math::belt::{binv, bsub, PRIME};
use nockchain_math::handle::{finalize_mary, new_handle_mut_mary};
use nockchain_math::noun_ext::NounMathExt;
use nockchain_math::structs::{HoonList, HoonMap};
use nockvm::jets::util::BAIL_FAIL;
use nockvm::jets::Result;
use nockvm::mem::NockStack;
use nockvm::noun::*;
use nockvm_macros::tas;
use zkvm_jetpack::form::mary::{Mary, MarySlice};

use super::utils::xeb;

// ::
// ::  $zero-map: see description
// ::
// ::    Nock 10 edits the noun so it has subject for the original noun and new-subject for the
// ::    new edited noun. Nock 0 is proved exactly like a nock 10 but with new-subject=subject.
// ::    So when recording a nock 0 you want to just pass subject in for new-subject.
// ::    Basically a nock 0 is a special case of nock 10 where the edited tree is the original tree.
// +$  zero-map  (map subject=* (map [axis=* new-subject=*] count=@))
// +$  decode-map  (map [formula=* head=* tail=*] count=@)
// +$  fock-return
//   $+  fock-return
//   $:  queue=(list *)
//       zeroes=zero-map
//       decodes=decode-map
//       [s=* f=*]
//       ::jutes=(list [@tas sam=* prod=*])
//   ==

// ++  bioz
fn bioz(b: u64) -> u64 {
    // ~/  %bioz
    // |=  b=belt
    // ^-  belt
    // ?:(=(b 0) 0 (binv b))
    if b == 0 {
        b
    } else {
        binv(b)
    }
}

// ++  num-randomizers  1
const NUM_RANDOMIZERS: u64 = 1;

fn header<const NUM_EXT_COLUMN_NAMES: u64>(stack: &mut NockStack) -> Noun {
    // ^-  header:table  ^~
    // TODO: Pull these automatically
    let r = [
        // :*  name:static:common
        D(tas!(b"memory")),
        //     p
        Atom::new(stack, PRIME).as_noun(),
        //     (lent basic-column-names:static:common)
        D(14),
        //     (lent ext-column-names:static:common)
        D(NUM_EXT_COLUMN_NAMES * 3),
        //     (lent mega-ext-column-names:static:common)
        D(8 * 3),
        //     (lent column-names:static:common)
        D(14 + NUM_EXT_COLUMN_NAMES * 3 + 8 * 3),
        //     num-randomizers
        D(NUM_RANDOMIZERS),
        // ==
    ];

    T(stack, &r)
}

pub fn build_v0_v1(stack: &mut NockStack, ret: Noun) -> Result {
    build_impl::<11>(stack, ret)
}

pub fn build_v2(stack: &mut NockStack, ret: Noun) -> Result {
    build_impl::<10>(stack, ret)
}

fn build_impl<const NUM_EXT_COLUMN_NAMES: u64>(stack: &mut NockStack, ret: Noun) -> Result {
    // ~/  %build
    // |=  return=fock-return
    let [_, zeroes, decodes, sf] = ret.uncell()?;
    let zeroes = HoonMap::try_from(zeroes).ok();
    let decodes = HoonMap::try_from(decodes).ok();
    // ^-  table-mary
    // =/  in  [s.return f.return]
    let [s, f] = sf.uncell()?;
    // =/  mult-mp=(map [* *] @)  (~(gut by zeroes.return) -.in *(map [* *] @))
    let mult_mp = zeroes.and_then(|v| v.get(stack, s)).unwrap_or(D(0));
    let mult_mp = HoonMap::try_from(mult_mp).ok();
    // =/  traversal  (rna-bfta ~[[s.return %.y] [f.return %.n]])
    let traversal = rna_bfta([(s, true), (f, false)].into_iter());
    // =/  len-traversal  (lent traversal)
    let len_traversal = traversal.len() as u64;
    // =/  end
    //   (init-bpoly ~[0 0 0 0 0 0 0 0 +(len-traversal) (binv +(len-traversal)) 0 0 0 0])
    let end: [u64; 14] =
        concat_arrays!([0; 8], [len_traversal + 1, binv(len_traversal + 1)], [0; 4]);
    // =-  [header (zing-bpolys mtx)]
    // %+  roll  traversal
    // |=  [mb=memory-bank ct=_len-traversal mtx=_`matrix`~[end]]
    // ^-  [belt matrix]
    let (_, mtx) = traversal
        .into_iter()
        .fold((len_traversal, vec![end]), |(ct, mut mtx), mb| {
            // :-  (dec ct)
            // :_  mtx
            // %-  init-bpoly
            // :~  1  ax.mb  (bioz ax.mb)  ?:(=(ax.mb 0) 0 1)
            let a1 = [1, mb.ax, bioz(mb.ax), if mb.ax == 0 { 0 } else { 1 }];
            //   ::
            //     ?:(?=(@ -.n.mb) -.n.mb 0)  ?:(?=(@ +.n.mb) +.n.mb 0)
            let a2 = [mb.n.head(), mb.n.tail()]
                .map(|v| v.as_atom().and_then(|v| v.as_u64()).unwrap_or(0));
            //   ::
            //     op-l.mb  op-r.mb  ct  (binv ct)
            let a3 = [mb.op_l, mb.op_r, ct, binv(ct)];
            //   ::
            let a4 = [
                //     ?:  !=(ax.mb 0)  0
                if mb.ax != 0 {
                    0
                }
                //     ?:((~(has by decodes.return) [n.mb -:n.mb +:n.mb]) 1 0)
                else {
                    let c = T(stack, &[mb.n.as_noun(), mb.n.head(), mb.n.tail()]);
                    if decodes.and_then(|v| v.get(stack, c)).is_some() {
                        1
                    } else {
                        0
                    }
                },
            ];
            //   ::
            let a5 = [
                //     ?:  =(ax.mb 1)  0
                if mb.ax == 1 {
                    0
                }
                //     (~(gut by mult-mp) [ax.mb -.in] 0)
                else {
                    let c = T(stack, &[D(mb.ax), s]);
                    mult_mp
                        .and_then(|v| v.get(stack, c))
                        .unwrap_or(D(0))
                        .as_atom()
                        .and_then(|v| v.as_u64())
                        .expect("Should fit in u64")
                },
            ];
            //   ::
            let a6 = [
                //     ?.  ?=(@ -.n.mb)  0
                if mb.n.head().is_cell() {
                    0
                }
                //     (~(gut by mult-mp) [(go-left ax.mb) -.in] 0)
                else {
                    let c = T(stack, &[D(go_left(mb.ax)), s]);
                    mult_mp
                        .and_then(|v| v.get(stack, c))
                        .unwrap_or(D(0))
                        .as_atom()
                        .and_then(|v| v.as_u64())
                        .expect("Should fit in u64")
                },
            ];
            //   ::
            let a7 = [
                //     ?.  ?=(@ +.n.mb)  0
                if mb.n.tail().is_cell() {
                    0
                }
                //     (~(gut by mult-mp) [(go-right ax.mb) -.in] 0)
                else {
                    let c = T(stack, &[D(go_right(mb.ax)), s]);
                    mult_mp
                        .and_then(|v| v.get(stack, c))
                        .unwrap_or(D(0))
                        .as_atom()
                        .and_then(|v| v.as_u64())
                        .expect("Should fit in u64")
                },
            ];
            let new: [u64; 14] = concat_arrays!(a1, a2, a3, a4, a5, a6, a7);
            // ==
            mtx.push(new);
            (ct - 1, mtx)
        });

    let header = header::<NUM_EXT_COLUMN_NAMES>(stack);

    let mlen = mtx.len();
    let (ret_ma, h_ma) = new_handle_mut_mary(stack, 14, mlen);
    h_ma.dat
        .chunks_mut(14)
        .zip(mtx.into_iter().rev())
        .for_each(|(a, b)| a.copy_from_slice(&b));
    let ma = finalize_mary(stack, 14, mlen, ret_ma);
    Ok(T(stack, &[header, ma]))
}

// +$  memory-bank
//   $:(n=^ ax=@ op-l=@ op-r=@)
#[derive(Clone, Copy, Debug)]
struct MemoryBank {
    n: Cell,
    ax: u64,
    op_l: u64,
    op_r: u64,
}

impl MemoryBank {
    fn as_noun(self, stack: &mut NockStack) -> Noun {
        T(
            stack,
            &[self.n.as_noun(), D(self.ax), D(self.op_l), D(self.op_r)],
        )
    }
}

// ::
// ++  go-left
//   ~/  %go-left
fn go_left(a: u64) -> u64 {
    // |=  a=@
    // (mul 2 a)
    2 * a
}

// ::
// ++  go-right
//   ~/  %go-right
fn go_right(a: u64) -> u64 {
    // |=  a=@
    // ?:(=(a 0) 0 (succ (mul 2 a)))
    if a == 0 {
        0
    } else {
        2 * a + 1
    }
}

// ::
// ::  rna-bfta: reversed non-atomic breadth-first traversal w axes
// ::
// ::    Returns the breadth-first traversal in reverse order bc the output is
// ::    piped to add-ions, which is most efficient if constructed from the bottom
// ::    of the tree to the top.

fn rna_bfta(tres: impl Iterator<Item = (Noun, bool)>) -> Vec<MemoryBank> {
    // ~/  %rna-bfta
    // |=  tres=(list [* ?])
    // ^-  (list memory-bank)
    // =/  qu=(list [^ @])
    //   %-  flop
    //   %+  roll  tres
    //   |=  [[n=* f=?] acc=(list [^ @])]
    let mut qu = tres
        .filter_map(|(n, f)| {
            // ?@  n  acc
            n.as_cell().ok().map(|n| (n, f))
        })
        .map(|(n, f)| {
            // ?:(f [[n 1] acc] [[n 0] acc])
            (n, f as u64)
        })
        .collect::<Vec<_>>();
    // =|  mbl=(list memory-bank)
    let mut mbl = vec![];
    let mut nu_qu = vec![];
    // |-
    // ?~  qu
    //   mbl
    while !qu.is_empty() {
        // =-  $(qu (flop nu-qu), mbl nu-mbl)
        // %+  roll  `(list [^ @])`qu
        // |=  [[n=^ a=@] nu-qu=(list [^ @]) nu-mbl=_mbl]
        for (n, a) in qu.drain(..) {
            // ^-  [(list [^ @]) (list memory-bank)]
            let head = n.head();
            let tail = n.tail();
            match (head.as_either_atom_cell(), tail.as_either_atom_cell()) {
                // ?@  -.n
                //   ?@  +.n
                (Either::Left(_), Either::Left(_)) => {
                    // [nu-qu [[n a 0 0] nu-mbl]]
                    mbl.push(MemoryBank {
                        n,
                        ax: a,
                        op_l: 0,
                        op_r: 0,
                    });
                }
                (Either::Left(_), Either::Right(t)) => {
                    // [[[+.n (go-right a)] nu-qu] [[n a 0 1] nu-mbl]]
                    nu_qu.push((t, go_right(a)));
                    mbl.push(MemoryBank {
                        n,
                        ax: a,
                        op_l: 0,
                        op_r: 1,
                    });
                }
                // ?@  +.n
                (Either::Right(h), Either::Left(_)) => {
                    // [[[-.n (go-left a)] nu-qu] [[n a 1 0] nu-mbl]]
                    nu_qu.push((h, go_left(a)));
                    mbl.push(MemoryBank {
                        n,
                        ax: a,
                        op_l: 1,
                        op_r: 0,
                    });
                }
                (Either::Right(h), Either::Right(t)) => {
                    // [[[+.n (go-right a)] [-.n (go-left a)] nu-qu] [[n a 1 1] nu-mbl]]
                    nu_qu.push((h, go_left(a)));
                    nu_qu.push((t, go_right(a)));
                    mbl.push(MemoryBank {
                        n,
                        ax: a,
                        op_l: 1,
                        op_r: 1,
                    });
                }
            }
        }
        core::mem::swap(&mut qu, &mut nu_qu);
    }
    mbl.reverse();
    mbl
}

pub fn rna_bfta_sam(stack: &mut NockStack, sam: Noun) -> Result {
    let iter = HoonList::try_from(sam)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let [n, f] = c.uncell().ok()?;
            f.as_direct().ok().map(|f| (n, f.data() == 0))
        });
    let ret = rna_bfta(iter);
    let mut ret = ret
        .into_iter()
        .map(|v| v.as_noun(stack))
        .collect::<Vec<_>>();
    ret.push(D(0));
    Ok(T(stack, &ret))
}

// TODO: move to seven.rs
fn height_mary(p: MarySlice) -> u32 {
    // ~/  %height-mary
    // |=  p=mary
    // ^-  @
    // ~+
    // =/  len  len.array.p
    // ?:  =(len 0)  0
    if p.len == 0 {
        0
    } else {
        // (bex (xeb (dec len)))
        1 << xeb((p.len - 1) as usize)
    }
}

pub fn pad(stack: &mut NockStack, sam: Noun) -> Result {
    // ~/  %pad
    // |=  table=table-mary
    let [header, p] = sam.uncell()?;
    let Ok(p) = MarySlice::try_from(p) else {
        return Err(BAIL_FAIL);
    };
    // NOTE: we rely on step being 14 for code to be correct (hoon doesn't check it, but it also
    // does rely on it).
    assert_eq!(p.step, 14);
    // ^-  table-mary
    // =/  height  (height-mary:tlib p.table)
    let height = height_mary(p);
    // ?:  =(height len.array.p.table)
    if height == p.len {
        // table
        return Ok(sam);
    }
    // =/  rows  p.table
    let mut rows = Mary {
        step: p.step,
        len: p.len,
        dat: p.dat.to_vec(),
    };
    // =/  len  len.array.rows
    let len = rows.len;
    // NOTE: already covered
    // ?:  =(height len)  table
    // =;  padding=mary
    //   table(p (~(weld ave rows) padding))
    // %-  zing-bpolys
    // %-  head
    // %^  spin  (range (sub height len))  (sub len 1)
    // |=  [i=@ ct=@]
    let mut ct = len as u64 - 1;
    for _ in 0..(height - len) {
        // :_  (bsub ct 1)
        // (init-bpoly ~[0 0 0 0 0 0 0 0 ct (binv ct) 0 0 0 0])
        let a1 = [0; 8];
        let a2 = [ct, binv(ct)];
        let a3 = [0; 4];
        ct = bsub(ct, 1);
        rows.dat.extend_from_slice(&a1);
        rows.dat.extend_from_slice(&a2);
        rows.dat.extend_from_slice(&a3);
        rows.len += 1;
    }

    let (ret_ma, h_ma) = new_handle_mut_mary(stack, rows.step as _, rows.len as _);
    h_ma.dat.copy_from_slice(&rows.dat);
    let ma = finalize_mary(stack, rows.step as _, rows.len as _, ret_ma);

    Ok(T(stack, &[header, ma]))
}
