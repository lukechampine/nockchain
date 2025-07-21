use either::Either;
use nbx_tip5::tip5::RATE;
use nockvm::jets::JetErr;
use nockvm::noun::{Atom, Noun, D};
use nockvm_macros::tas;
use tracing::log::*;

#[cfg(feature = "gpu")]
use super::gpu::{self, Submittable};
use super::three::{hash_10, hash_varlen_padded};
use zkvm_jetpack::form::mary::MarySlice;
use zkvm_jetpack::form::math::tip5::DIGEST_LENGTH;
use zkvm_jetpack::form::{Belt, Element, Felt, Melt};
use zkvm_jetpack::hand::structs::HoonList;
use zkvm_jetpack::jets::utils::jet_err;
use zkvm_jetpack::noun::noun_ext::NounExt;

// 64MB in melts
const MAX_CHUNK_SIZE: usize = 0x4000000 / core::mem::size_of::<Melt>();

pub type NounDigest<T = Melt> = [T; 5];

pub trait Pushable {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr>;
}

impl Pushable for Noun {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr> {
        engine.push(stage, self)
    }
}

impl<T: Into<Melt> + Copy> Pushable for NounDigest<T> {
    fn push(self, engine: &mut HashEngine, stage: usize) -> core::result::Result<usize, JetErr> {
        Ok(engine.push_hash(stage, self))
    }
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

#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct ReduceOp {
    pub source: u32,
    pub destination: u32,
}

impl ReduceOp {
    fn reduce_fixed(
        self,
        inp_start: usize,
        input: &[Melt],
        out_ptr: *mut Melt,
        out_len: u32,
        out_off: usize,
    ) {
        let dig = hash_10(
            input[(self.source as usize - inp_start)
                ..((self.source as usize) + DIGEST_LENGTH * 2 - inp_start)]
                .try_into()
                .unwrap(),
        );

        let dest = self.destination as usize - out_off;
        debug_assert!((out_len as usize) >= dest + DIGEST_LENGTH);
        unsafe { core::slice::from_raw_parts_mut(out_ptr.add(dest), DIGEST_LENGTH) }
            .copy_from_slice(&dig);
    }
}

#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct VariableReduceOp {
    pub inner: ReduceOp,
    pub len: u32,
}

impl VariableReduceOp {
    fn reduce(
        self,
        inp_start: usize,
        input: &[Melt],
        out_ptr: *mut Melt,
        out_len: u32,
        out_off: usize,
    ) {
        let Self { inner, len } = self;

        let dig = hash_varlen_padded(
            &input
                [(inner.source as usize - inp_start)..((inner.source + len) as usize - inp_start)],
        );

        let dest = inner.destination as usize - out_off;
        debug_assert!((out_len as usize) >= dest + DIGEST_LENGTH);
        unsafe { core::slice::from_raw_parts_mut(out_ptr.add(dest), DIGEST_LENGTH) }
            .copy_from_slice(&dig);
    }
}

pub struct ReduceChunkSlice<'a> {
    pub ops_variable: &'a [VariableReduceOp],
    pub ops_fixed: &'a [ReduceOp],
    pub out_start: usize,
    pub out: &'a mut [Melt],
}

impl<'a> ReduceChunkSlice<'a> {
    #[tracing::instrument(skip_all)]
    fn reduce(self, inp_start: usize, inp: &[Melt]) {
        // TODO: multithread/GPU this.
        use rayon::prelude::*;
        struct MeltSlice(*mut Melt);
        unsafe impl Send for MeltSlice {}
        unsafe impl Sync for MeltSlice {}
        let out_ptr = MeltSlice(self.out.as_mut_ptr());
        let out_len = self.out.len() as u32;
        let out_off = self.out_start;
        self.ops_fixed.into_iter().for_each(|op| {
            let out = &out_ptr;
            op.reduce_fixed(inp_start, inp, out.0, out_len, out_off);
        });
        self.ops_variable.into_iter().for_each(|op| {
            let out = &out_ptr;
            op.reduce(inp_start, inp, out.0, out_len, out_off);
        });
    }

    pub fn min_source(&self) -> Option<usize> {
        let var = self.ops_variable.first().map(|v| v.inner.source as usize);
        let fix = self.ops_fixed.first().map(|v| v.source as usize);
        var.zip(fix)
            .map(|(a, b)| core::cmp::min(a, b))
            .or(var)
            .or(fix)
    }

    pub fn max_source(&self) -> Option<usize> {
        let var = self
            .ops_variable
            .last()
            .map(|v| (v.inner.source + v.len) as usize);
        let fix = self
            .ops_fixed
            .last()
            .map(|v| (v.source as usize + 2 * DIGEST_LENGTH));
        var.zip(fix)
            .map(|(a, b)| core::cmp::max(a, b))
            .or(var)
            .or(fix)
    }

    fn verify_ops(&self) {
        for op in self.ops_fixed {
            let start = op.destination as usize;
            let end = start + DIGEST_LENGTH;
            assert!(start >= self.out_start);
            assert!(end <= self.out_start + self.out.len());
        }
    }

    pub fn split_at_inp(self, inp_at: usize) -> (Self, Self) {
        self.verify_ops();
        trace!(
            "Split at out_start={} | inp_at={inp_at} | out={} ops_fixed={} ops_var={} | {:?}",
            self.out_start,
            self.out.len(),
            self.ops_fixed.len(),
            self.ops_variable.len(),
            self.ops_fixed.last(),
        );
        let ops_fixed = self
            .ops_fixed
            .binary_search_by_key(&inp_at, |e| (e.source as usize) + DIGEST_LENGTH * 2)
            // We want _the next_ element to split at
            .map(|v| v + 1)
            .unwrap_or_else(|v| v);
        let (of1, of2) = self.ops_fixed.split_at(ops_fixed);
        trace!(
            "{:?} | {:?} | {} {}",
            of1.last(),
            of2.first(),
            of1.len(),
            of2.len()
        );

        let ops_variable = self
            .ops_variable
            .binary_search_by_key(&inp_at, |e| ((e.inner.source + e.len) as usize))
            // We want _the next_ element to split at
            .map(|v| v + 1)
            .unwrap_or_else(|v| v);
        let (ov1, ov2) = self.ops_variable.split_at(ops_variable);

        let mut out = self.out.len();
        trace!("out={out}");
        if let Some(op) = of2.first() {
            out = core::cmp::min(out, op.destination as usize - self.out_start);
            trace!("out2={out}");
        }
        if let Some(op) = ov2.first() {
            out = core::cmp::min(out, op.inner.destination as usize - self.out_start);
            trace!("out3={out}");
        }
        let (o1, o2) = self.out.split_at_mut(out);
        trace!("o1={} o2={} out={out}", o1.len(), o2.len());

        (
            Self {
                ops_variable: ov1,
                ops_fixed: of1,
                out_start: self.out_start,
                out: o1,
            },
            Self {
                ops_variable: ov2,
                ops_fixed: of2,
                out_start: self.out_start + out,
                out: o2,
            },
        )
    }
}

#[derive(Default)]
pub struct ReduceChunk {
    pub ops_variable: Vec<VariableReduceOp>,
    pub ops_fixed: Vec<ReduceOp>,
    pub out_start: usize,
    pub out: Vec<Melt>,
}

impl ReduceChunk {
    fn reduce(mut self, inp_start: usize, inp: &[Melt]) -> Vec<Melt> {
        self.as_slice().reduce(inp_start, inp);
        self.out
    }

    pub fn as_slice(&mut self) -> ReduceChunkSlice {
        ReduceChunkSlice {
            ops_variable: &self.ops_variable,
            ops_fixed: &self.ops_fixed,
            out_start: self.out_start,
            out: &mut self.out,
        }
    }

    fn push_variable(&mut self, a: usize, len: usize) -> usize {
        let ret = self.out.len();
        self.out.resize(ret + DIGEST_LENGTH, Melt(0));
        let ret = ret + self.out_start;
        self.ops_variable.push(VariableReduceOp {
            inner: ReduceOp {
                source: a as u32,
                destination: ret as u32,
            },
            len: len as u32,
        });
        ret
    }

    fn push_fixed(&mut self, a: usize, b: usize) -> usize {
        assert_eq!(a + DIGEST_LENGTH, b);
        let ret = self.out.len();
        self.out.resize(ret + DIGEST_LENGTH, Melt(0));
        let ret = ret + self.out_start;
        self.ops_fixed.push(ReduceOp {
            source: a as u32,
            destination: ret as u32,
        });
        ret
    }
}

#[derive(Default)]
pub struct ReduceStage {
    pub chunks: Vec<ReduceChunk>,
}

impl ReduceStage {
    #[tracing::instrument(skip_all)]
    fn reduce<'a, I: Iterator<Item = &'a [Melt]>>(mut self, inps: I) -> Vec<Vec<Melt>> {
        let pairs = self.split_chunks(inps);

        use rayon::prelude::*;

        pairs
            .into_iter()
            .for_each(|(_, inp_at, input, _, chunk)| chunk.reduce(inp_at, input));

        self.chunks.into_iter().map(|v| v.out).collect()
    }

    pub fn split_chunks<'a, I: Iterator<Item = &'a [Melt]>>(
        &mut self,
        inps: I,
    ) -> Vec<(usize, usize, &'a [Melt], usize, ReduceChunkSlice<'_>)> {
        #[cfg(debug_assertions)]
        {
            trace!("Verify");
            self.chunks
                .iter_mut()
                .for_each(|c| c.as_slice().verify_ops());
            trace!("Verified");
        }

        let mut inps = inps.enumerate();
        let mut chunks = self.chunks.iter_mut().map(|c| c.as_slice()).enumerate();
        let mut chunk = None;

        let mut inp_idx = usize::MAX;
        let mut inp = &[][..];
        let mut inp_at = 0;

        // Split-up pairs of (inp_at, input, chunk)
        let mut pairs = vec![];

        debug!("Reduce stage");

        while let Some((cid, c)) = chunk.take().or_else(|| chunks.next()) {
            if inp.is_empty() {
                (inp_idx, inp) = inps.next().unwrap_or((usize::MAX, &[]));
            }
            trace!(
                "Split at inp {} {} {}",
                inp_at + inp.len(),
                inp_at,
                inp.len()
            );
            let (c, nc) = c.split_at_inp(inp_at + inp.len());
            let ms = if let Some(ms) = nc.min_source() {
                if ms == inp_at {
                    panic!("First chunk is empty (at chunk {cid})! This usually means that the hash inputs are split across multiple input buffers. This is illegal and shouldn't happen (potential ops in question: {:?} | {:?}). See - reserve_pair", nc.ops_fixed.first(), nc.ops_variable.first());
                }

                trace!("MS ms={ms} inp_at={inp_at} ms-inp_at={} inp={} | cout={} cfixed={} cvar={} cos={} | ncout={} ncfixed={} ncvar={} ncos={}", ms - inp_at, inp.len(), c.out.len(), c.ops_fixed.len(), c.ops_variable.len(), c.out_start, nc.out.len(), nc.ops_fixed.len(), nc.ops_variable.len(), nc.out_start);
                chunk = Some((cid, nc));
                ms
            } else {
                c.max_source().unwrap_or(inp_at + inp.len())
            };

            let (cur_inp, next_inp) = inp.split_at(ms - inp_at);
            trace!("ci={} ni={}", cur_inp.len(), next_inp.len());
            pairs.push((inp_idx, inp_at, cur_inp, cid, c));
            inp_at = ms;
            inp = next_inp;
        }

        trace!("Split stage up");

        pairs
    }

    fn reserve_in_new_chunk(&mut self, len: usize, out_start: usize) -> &mut ReduceChunk {
        assert!(
            len <= MAX_CHUNK_SIZE,
            "{len:x} out of bounds of {MAX_CHUNK_SIZE:x}"
        );
        self.chunks.push(ReduceChunk {
            out_start,
            ..Default::default()
        });
        self.chunks.last_mut().unwrap()
    }

    fn reserve_in_chunk(&mut self, len: usize) -> &mut ReduceChunk {
        if let Some(c) = self.chunks.last_mut() {
            if c.out.len() + len <= MAX_CHUNK_SIZE {
                // NOTE: need to re-borrow for borrowchk to be happy
                self.chunks.last_mut().unwrap()
            } else {
                let out_start = c.out_start + c.out.len();
                self.reserve_in_new_chunk(len, out_start)
            }
        } else {
            self.reserve_in_new_chunk(len, 0)
        }
    }

    fn push_variable(&mut self, a: usize, len: usize) -> usize {
        let chunk = self.reserve_in_chunk(DIGEST_LENGTH);
        chunk.push_variable(a, len)
    }

    fn push_fixed(&mut self, a: usize, b: usize) -> usize {
        let chunk = self.reserve_in_chunk(DIGEST_LENGTH);
        chunk.push_fixed(a, b)
    }

    fn push_const(&mut self, d: &[Melt]) -> usize {
        let chunk = self.reserve_in_chunk(d.len());
        let ret = chunk.out_start + chunk.out.len();
        chunk.out.extend_from_slice(d);
        ret
    }
}

fn padded_chunk(len: usize) -> usize {
    (len + RATE) / RATE * RATE
}

fn pad_chunk(buf: &mut Vec<Melt>, len: usize) {
    buf.push(Melt::one());
    buf.resize(buf.len() + padded_chunk(len) - len - 1, Melt::zero())
}

pub struct HashEngine {
    stages: Vec<ReduceStage>,
    out_stages: usize,
}

impl Default for HashEngine {
    fn default() -> Self {
        Self {
            stages: vec![],
            out_stages: 1,
        }
    }
}

impl HashEngine {
    pub fn set_out_stages(&mut self, stages: usize) {
        self.out_stages = stages;
    }

    pub fn push_varlen(&mut self, stage: usize, m: impl Iterator<Item = Melt>) -> (usize, usize) {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage = self.stages.get_mut(stage).unwrap();
        let mut buf = m.collect::<Vec<_>>();

        let buf_len = buf.len();
        pad_chunk(&mut buf, buf_len);

        (stage.push_const(&buf), buf.len())
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

        let (source, pushed_len) = self.push_varlen(stage + 1, melts);
        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_variable(source, pushed_len);

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

        let l = l.collect::<Vec<_>>();
        let len = l.len();
        let orig_total_len = 2 + (DIGEST_LENGTH + 10) * l.len();
        let total_len = padded_chunk(orig_total_len);

        let stage0 = self.stages.get_mut(stage).unwrap();
        let chunk = stage0.reserve_in_chunk(total_len);
        let ret = chunk.out_start + chunk.out.len();
        chunk.out.push(Melt(0));

        let mut next = ret + 1;
        for h in l {
            let out = h.push(self, stage)?;
            assert_eq!(out, next);
            next = out + DIGEST_LENGTH;
        }

        let stage = self.stages.get_mut(stage).unwrap();
        let chunk = stage.chunks.last_mut().unwrap();
        chunk.out[ret] = Melt::from_u64((len * 5 + 1) as _);
        // Push dyck shape
        chunk.out.push(Melt::zero());
        let shape = [0, 0, 1, 0, 1, 0, 1, 0, 1, 1].map(Melt::from_u64);
        chunk.out.extend(core::iter::repeat_n(shape, len).flatten());

        pad_chunk(&mut chunk.out, orig_total_len);

        assert_eq!(chunk.out.len() - total_len, ret - chunk.out_start);

        Ok((ret, total_len))
    }

    pub fn destruct(self) -> (Vec<ReduceStage>, usize) {
        (self.stages, self.out_stages)
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce_cpu(mut self) -> Vec<NounDigest> {
        let mut out_len = 0;
        let out_pos = self
            .stages
            .iter()
            .take(self.out_stages)
            .map(|v| {
                let sum = v.chunks.iter().map(|v| v.out.len()).sum::<usize>();
                assert!(sum % DIGEST_LENGTH == 0);
                let r = out_len;
                out_len += sum / DIGEST_LENGTH;
                r
            })
            .collect::<Vec<_>>();
        let mut out = vec![NounDigest::default(); out_len];
        assert!(out_len <= MAX_CHUNK_SIZE);

        let mut cur = vec![];
        //let mut cnt = 0;
        while let Some(stage) = self.stages.pop() {
            let cur_stage = self.stages.len();
            //println!("Layer {cnt}: {} {}", stage.ops.len(), stage.out.len());
            //cnt += 1;
            //let t = std::time::Instant::now();
            cur = stage.reduce(cur[..].iter().map(|v: &Vec<_>| v.as_ref()));

            if cur_stage < self.out_stages {
                let stage_len = cur.iter().map(|v| v.len() / DIGEST_LENGTH).sum::<usize>();
                out[out_pos[cur_stage]..(out_pos[cur_stage] + stage_len)]
                    .iter_mut()
                    .zip(cur.iter().flat_map(|v| {
                        v.chunks(DIGEST_LENGTH)
                            .map(|v| NounDigest::try_from(v).unwrap())
                    }))
                    .for_each(|(a, b)| *a = b);
            }
            //println!("{:.02}s", t.elapsed().as_secs_f64())
        }
        out
    }

    pub fn reduce(self) -> Vec<NounDigest> {
        if self.stages.is_empty() {
            return vec![];
        }

        #[cfg(feature = "gpu")]
        if gpu::should_use_gpu() {
            Submittable::gpu_process(self)
        } else {
            self.reduce_cpu()
        }

        #[cfg(not(feature = "gpu"))]
        self.reduce_cpu()
    }

    pub fn push_mary(&mut self, stage: usize, ma: MarySlice) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        //   %-  hash-hashable

        //   :-  leaf+step.p.h
        self.reserve_pair(stage + 1);
        let step = self.push_noun(stage + 1, D(ma.step as _)).unwrap();

        //   :-  leaf+len.array.p.h
        self.reserve_pair(stage + 2);
        let len = self.push_noun(stage + 2, D(ma.len as _)).unwrap();

        //   hash+(hash-belts-list (bpoly-to-list array:(~(change-step ave p.h) 1)))
        let (hash_src, pushed_len) =
            self.push_varlen(stage + 3, ma.dat.iter().copied().map(Melt::from_u64));
        let stage2 = self.stages.get_mut(stage + 2).unwrap();
        let hash = stage2.push_variable(hash_src, pushed_len);

        let stage1 = self.stages.get_mut(stage + 1).unwrap();
        let arr = stage1.push_fixed(len, hash);

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_fixed(step, arr);

        ret
    }

    pub fn push_hash<T: Into<Melt> + Copy>(&mut self, stage: usize, h: NounDigest<T>) -> usize {
        self.push_hashes(stage, &[h])
    }

    pub fn push_hashes<T: Into<Melt> + Copy>(
        &mut self,
        stage: usize,
        h: &[NounDigest<T>],
    ) -> usize {
        let stage = self.stages.get_mut(stage).unwrap();
        let chunk = stage.reserve_in_chunk(h.len() * DIGEST_LENGTH);
        let ret = chunk.out_start + chunk.out.len();

        chunk.out.extend(h.iter().flat_map(|v| v.map(Into::into)));
        ret
    }

    pub fn push_list<T: Pushable>(
        &mut self,
        stage: usize,
        l: impl Iterator<Item = T>,
    ) -> core::result::Result<usize, JetErr> {
        let (source, total_len) = self.push_list_inner(stage + 1, l)?;

        let stage = self.stages.get_mut(stage).unwrap();
        let ret = stage.push_variable(source, total_len);

        Ok(ret)
    }

    pub fn ensure_stages(&mut self, stage: usize) {
        while self.stages.len() <= stage {
            self.stages.push(ReduceStage::default());
        }
    }

    pub fn reserve_pair(&mut self, stage: usize) {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(ReduceStage::default());
        }

        let stage = self.stages.get_mut(stage).unwrap();
        stage.reserve_in_chunk(2 * DIGEST_LENGTH);
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
                self.reserve_pair(stage + 1);
                let a = self.push(stage + 1, h.head())?;
                let b = self.push(stage + 1, h.tail())?;
                Ok(self.push_pair(stage, a, b))
            }
        }
    }
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

pub fn leaf_sequence_impl<T: FromAtom>(mut t: Noun) -> core::result::Result<Vec<T>, JetErr> {
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
