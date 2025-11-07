use std::collections::BTreeMap;
use std::sync::Arc;

use nbx_tip5::melt::Melt;
use nockchain_math::hadamard::p_hadamard_inplace;
use nockchain_math::poly::*;
use nockchain_math::poly_ext::*;
use rayon::prelude::*;

#[cfg(feature = "gpu")]
use super::gpu;
use crate::engine::Engine;

// 64MB in melts/belts
pub const MAX_CHUNK_SIZE: usize = 0x4000000 / core::mem::size_of::<u64>();

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, Ord, Eq, PartialEq, PartialOrd)]
#[repr(C)]
pub struct SubstituteOp {
    pub chunk: u32,
    pub exp: u32,
}

#[derive(Clone)]
pub struct SubstituteMulStage<E: ElementEx> {
    pub vars: Vec<SubstituteOp>,
    pub coms: Vec<SubstituteOp>,
    pub scal: E,
}

impl<E: ElementEx> SubstituteMulStage<E> {
    pub fn new(scal: E) -> Self {
        Self {
            vars: vec![],
            coms: vec![],
            scal,
        }
    }
}

#[derive(Clone)]
pub struct SubstituteIter<'a, E: ElementEx> {
    pub muls: Vec<SubstituteMulStage<E>>,
    pub zero_traces: Arc<[bool]>,
    pub traces: PolySlice<'a, E>,
}

impl<'a, E: ElementEx> SubstituteIter<'a, E> {
    fn new(traces: PolySlice<'a, E>, zero_traces: Arc<[bool]>) -> Self {
        Self {
            muls: vec![],
            zero_traces,
            traces,
        }
    }
}

impl<E: ElementEx> SubstituteIter<'_, E> {
    fn reduce(self, inp: &[impl AsRef<[E]> + Send + Sync], out: &mut [E]) {
        let out_len = out.len();
        let inp_chunks = MAX_CHUNK_SIZE / out_len;

        if self.muls.is_empty() {
            return;
        }

        // Because all operations are sorted (to deduplicate them), if there is an entry with only
        //  a scalar, it will be at the start of the operations. By explicitly handling this case,
        //  `out.len()` multiplications and additions are skipped.
        let mut scalar = E::zero();
        if self.muls[0].coms.is_empty() && self.muls[0].vars.is_empty() {
            scalar = self.muls[0].scal;
        }

        let r = self
            .muls
            .into_par_iter()
            .with_min_len(128)
            .filter_map(|m| {
                if m.coms.is_empty() && m.vars.is_empty() {
                    return None;
                }

                let result = m
                    .coms
                    .into_iter()
                    .map(|SubstituteOp { chunk, exp }| {
                        (
                            &inp[(chunk as usize) / inp_chunks]
                                .as_ref()
                                .split_at(out_len * ((chunk as usize) % inp_chunks))
                                .1[..out_len],
                            exp as u64,
                        )
                    })
                    .chain(m.vars.into_iter().map(|SubstituteOp { chunk, exp }| {
                        let var = self.traces.0.split_at(out_len * (chunk as usize)).1;
                        let var = &var[..out_len];
                        (var, exp as u64)
                    }))
                    .fold(PolyVec(vec![m.scal; out_len]), |mut acc, (o, exp)| {
                        debug_assert_eq!(o.len(), acc.0.len());
                        debug_assert!(o.len() % 16 == 0);
                        debug_assert!(acc.0.len() % 16 == 0);
                        let acc_len = acc.0.len() & !0xf;
                        let a = acc.0.split_at_mut(acc_len).0;
                        let b = o.split_at(acc_len).0;
                        for _ in 0..exp {
                            p_hadamard_inplace(a, b);
                        }
                        acc
                    });

                Some(result)
            })
            .reduce(
                || PolyVec(vec![scalar; out_len]),
                |mut acc, o| {
                    padd_in_place(&mut acc.0, &o.0);
                    acc
                },
            );
        out.copy_from_slice(&r.0);
    }
}

#[derive(Clone)]
pub struct SubstituteStage<'a, E: ElementEx> {
    pub iters: Vec<SubstituteIter<'a, E>>,
    pub out: Vec<Vec<E>>,
}

impl<E: ElementEx> Default for SubstituteStage<'_, E> {
    fn default() -> Self {
        Self {
            iters: vec![],
            out: vec![],
        }
    }
}

impl<E: ElementEx> SubstituteStage<'_, E> {
    #[tracing::instrument(skip_all)]
    fn reduce(mut self, poly_len: usize, inp: &[impl AsRef<[E]> + Send + Sync]) -> Vec<Vec<E>> {
        let t = self
            .iters
            .into_iter()
            .zip(self.out.iter_mut().flat_map(|m| m.chunks_mut(poly_len)))
            .collect::<Vec<_>>();

        t.into_iter().for_each(|(i, o)| i.reduce(inp, o));

        self.out
    }
}

#[derive(Clone)]
pub struct SubstituteEngine<'a, E: ElementEx> {
    stages: Vec<SubstituteStage<'a, E>>,
    poly_len: usize,
    zero_trace_cache: BTreeMap<(usize, usize), Arc<[bool]>>,
}

impl Engine for SubstituteEngine<'_, Melt> {
    type Output = (Vec<Vec<Melt>>, usize);

    #[tracing::instrument(skip_all)]
    fn reduce_cpu(self) -> Self::Output {
        self.reduce_cpu()
    }

    #[tracing::instrument(skip_all)]
    #[cfg(feature = "gpu")]
    fn reduce_gpu(self, gpu: gpu::GpuHandle) -> Self::Output {
        use super::gpu::Submittable;
        let poly_len = self.poly_len;
        (Submittable::gpu_process(self, gpu), poly_len)
    }
}

impl<'a, E: ElementEx> SubstituteEngine<'a, E> {
    pub fn new(height: u64) -> Self {
        let poly_len = (height as usize) * 4;
        assert!(
            poly_len <= MAX_CHUNK_SIZE,
            "{poly_len:x} is larger than {MAX_CHUNK_SIZE:x}"
        );
        Self {
            stages: vec![],
            poly_len,
            zero_trace_cache: BTreeMap::new(),
        }
    }

    pub fn destruct(self) -> (Vec<SubstituteStage<'a, E>>, usize) {
        (self.stages, self.poly_len)
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce_cpu(mut self) -> (Vec<Vec<E>>, usize) {
        let mut cur = vec![];
        while let Some(stage) = self.stages.pop() {
            cur = stage.reduce(self.poly_len, &cur);
        }
        (cur, self.poly_len)
    }

    pub fn poly_len(&self) -> usize {
        self.poly_len
    }

    pub fn ensure_stages(&mut self, stages: usize) {
        while self.stages.len() <= stages {
            self.stages.push(Default::default());
        }
    }

    pub fn push_iter(
        &mut self,
        stage: usize,
        out: Option<PolySlice<E>>,
        traces: PolySlice<'a, E>,
    ) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(Default::default());
        }
        let stage = &mut self.stages[stage];
        let ret = stage.iters.len();
        let zero_traces = self
            .zero_trace_cache
            .entry((traces.0.as_ptr() as usize, traces.0.len()))
            .or_insert_with(|| {
                let zt = traces
                    .0
                    .chunks(self.poly_len)
                    .map(|c| c.iter().all(|v| v.is_zero()))
                    .collect::<Vec<_>>();
                Arc::from(&*zt)
            })
            .clone();
        stage.iters.push(SubstituteIter::new(traces, zero_traces));

        let fits_in_last = stage
            .out
            .last()
            .map(|v| v.len() + self.poly_len <= MAX_CHUNK_SIZE)
            .unwrap_or(false);
        if !fits_in_last {
            stage.out.push(vec![]);
        }
        let lout = stage.out.last_mut().unwrap();

        if let Some(out) = out {
            assert_eq!(out.len(), self.poly_len);
            lout.extend_from_slice(out.0);
        } else {
            lout.resize(lout.len() + self.poly_len, E::zero());
        }
        ret
    }

    pub fn push_mul(&mut self, stage: usize, iter: usize, mul: SubstituteMulStage<E>) {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(Default::default());
        }
        let stage = &mut self.stages[stage];
        let iter = &mut stage.iters[iter];

        // If the mul stage hits any zero-trace, we can filter it out
        if mul
            .vars
            .iter()
            .any(|v| iter.zero_traces.get(v.chunk as usize) == Some(&true))
        {
            return;
        }

        // Use the observation the combination of `vars` & `coms` is not unique such that multiple
        // multiplications can be combined if they are of the following form;
        //
        // scal1 × chunk_A^exp_A × chunk_B^exp_B
        // scal2 × chunk_A^exp_A × chunk_B^exp_B
        //
        // Can be deduplicated into a single multiplication in the form of;
        // (scal1 + scal2) × chunk_A^exp_A × chunk_B^exp_B

        let mut mul = mul;
        // NOTE: Hadamard product operations are element-wise multiplication. This operation
        //      is commutative and associative. Which means that the order of operations can
        //      be shifted. By sorting the operations, they are consistent for comparison.
        mul.vars.sort_unstable();
        mul.coms.sort_unstable();

        match iter
            .muls
            .binary_search_by_key(&(&mul.vars, &mul.coms), |mul| (&mul.vars, &mul.coms))
        {
            Ok(index) => {
                iter.muls[index].scal += mul.scal;
            }
            Err(index) => {
                iter.muls.insert(index, mul);
            }
        }
    }
}
