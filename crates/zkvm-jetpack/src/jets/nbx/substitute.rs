use crate::form::math::poly::*;
use crate::form::poly::Poly;
use crate::form::{ElementEx, PolySlice, PolyVec};

pub struct SubstituteMulStage<E: ElementEx> {
    pub vars: Vec<(usize, u64)>,
    pub coms: Vec<(usize, u64)>,
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

struct SubstituteIter<'a, E: ElementEx> {
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
    fn reduce(self, inp: &[E], out: &mut [E]) {
        let out_len = out.len();
        self.muls
            .into_iter()
            .map(|m| {
                let acc = PolyVec(vec![m.scal; out_len]);

                // NOTE: never parallel iter here, because it is slow
                m.coms
                    .into_iter()
                    .map(|(i, exp)| (&inp.split_at(out_len * i).1[..out_len], exp))
                    .chain(m.vars.into_iter().map(|(i, exp)| {
                        let var = self.traces.0.split_at(out_len * i).1;
                        let var = &var[..out_len];
                        (var, exp)
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

struct SubstituteStage<'a, E: ElementEx> {
    pub iters: Vec<SubstituteIter<'a, E>>,
    pub out: Vec<E>,
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
    fn reduce(mut self, poly_len: usize, inp: &[E]) -> Vec<E> {
        self.iters
            .into_iter()
            .zip(self.out.chunks_mut(poly_len))
            .for_each(|(i, o)| i.reduce(inp, o));
        self.out
    }
}

pub struct SubstituteEngine<'a, E: ElementEx> {
    stages: Vec<SubstituteStage<'a, E>>,
    poly_len: usize,
}

impl<'a, E: ElementEx> SubstituteEngine<'a, E> {
    pub fn new(height: u64) -> Self {
        Self {
            stages: vec![],
            poly_len: (height as usize) * 4,
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce(mut self) -> (Vec<E>, usize) {
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

    pub fn push_iter(&mut self, stage: usize, out: Option<PolySlice<E>>, traces: PolySlice<'a, E>) -> usize {
        if self.stages.len() <= stage {
            assert_eq!(self.stages.len(), stage);
            self.stages.push(Default::default());
        }
        let stage = &mut self.stages[stage];
        let ret = stage.iters.len();
        stage.iters.push(SubstituteIter::new(traces));
        if let Some(out) = out {
            assert_eq!(out.len(), self.poly_len);
            stage.out.extend_from_slice(out.0);
        } else {
            stage.out.resize(stage.out.len() + self.poly_len, E::zero());
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
