use std::sync::OnceLock;

use nbx_tip5::melt::Melt;

use zkvm_jetpack::form::math::poly::*;
use zkvm_jetpack::form::poly::Poly;
use zkvm_jetpack::form::{ElementEx, PolySlice, PolyVec};

#[cfg(feature = "gpu")]
use super::gpu;

// 64MB in melts/belts
pub const MAX_CHUNK_SIZE: usize = 0x4000000 / core::mem::size_of::<u64>();

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
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
    pub traces: PolySlice<'a, E>,
}

impl<'a, E: ElementEx> SubstituteIter<'a, E> {
    fn new(traces: PolySlice<'a, E>) -> Self {
        Self {
            muls: vec![],
            traces,
        }
    }
}

impl<E: ElementEx> SubstituteIter<'_, E> {
    fn reduce(self, inp: &[impl AsRef<[E]>], out: &mut [E]) {
        let out_len = out.len();
        let inp_chunks = MAX_CHUNK_SIZE / out_len;
        self.muls
            .into_iter()
            .map(|m| {
                let acc = PolyVec(vec![m.scal; out_len]);

                // NOTE: never parallel iter here, because it is slow
                m.coms
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
                    .fold(acc, |mut acc, (o, exp)| {
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
                    })
            })
            .fold(
                out,
                /*|| PolyVec(vec![E::zero(); out_len]),*/
                |acc, o| {
                    padd_in_place(acc, &o.0);
                    acc
                },
            );
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
    fn reduce(mut self, poly_len: usize, inp: &[impl AsRef<[E]>]) -> Vec<Vec<E>> {
        self.iters
            .into_iter()
            .zip(self.out.iter_mut().flat_map(|m| m.chunks_mut(poly_len)))
            .for_each(|(i, o)| i.reduce(inp, o));
        self.out
    }
}

#[derive(Clone)]
pub struct SubstituteEngine<'a, E: ElementEx> {
    stages: Vec<SubstituteStage<'a, E>>,
    poly_len: usize,
}

impl SubstituteEngine<'_, Melt> {
    #[tracing::instrument(skip_all)]
    #[cfg(feature = "gpu")]
    pub fn reduce_gpu(self) -> (Vec<Vec<Melt>>, usize) {
        use super::gpu::Submittable;
        let poly_len = self.poly_len;
        (Submittable::gpu_process(self), poly_len)
    }

    pub fn reduce(self) -> (Vec<Vec<Melt>>, usize) {
        #[cfg(feature = "gpu")]
        if gpu::should_use_gpu() {
            self.reduce_gpu()
        } else {
            self.reduce_cpu()
        }

        #[cfg(not(feature = "gpu"))]
        self.reduce_cpu()
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
        stage.iters.push(SubstituteIter::new(traces));

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
        iter.muls.push(mul);
    }
}
