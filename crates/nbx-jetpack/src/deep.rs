use std::sync::Arc;
use crate::log::*;

use nbx_tip5::base::binv;
use zkvm_jetpack::form::math::poly::{
    p_fft_twiddles, p_hadamard_inplace, p_ntt_twiddled, p_ntt_twiddles, pscal_inplace,
};
use zkvm_jetpack::form::{Belt, ElementEx, FPolySlice, FPolyVec, Felt, PolySlice, PolyVec};
use crate::new_fpoly;
use crate::two::{
    con_mon, fdegree, fpadd, fpscal, fpsub, id_fpoly, pinv_mod_x_to, zero_fpoly, zeroextend_slice,
};
use crate::utils::{scag_vec, xeb};
use rayon::prelude::*;

#[derive(Clone)]
pub struct WeightedDivConst {
    pub d: Felt,
    pub dq: usize,
    pub deg_prod: usize,
    pub pinned_ntt: FPolyVec,
    pub twiddles: Arc<[Vec<Felt>]>,
    pub inv_len: Felt,
    pub ifft_twiddles: Arc<[Vec<Felt>]>,
}

impl WeightedDivConst {
    fn new(q: FPolyVec, pl: usize) -> Self {
        let (d, g) = con_mon(q);
        let dg = fdegree((&g).into());

        let mut rg = g;
        rg.0.reverse();

        // fdegree(p)
        let df = pl - 1;

        let dq = df - dg;
        let pinned = pinv_mod_x_to(dq + 1, (&rg).into());

        // =*  deg-p  len.fp
        let deg_p = pinned.0.len();
        // =*  deg-q  len.fq
        let deg_q = pl;

        // =/  deg-prod  (bex (xeb (dec (add deg-p deg-q))))
        let deg_prod = 1 << xeb(deg_p + deg_q - 1);

        let a = zeroextend_slice(pinned, deg_prod, Felt::zero());
        let twiddles: Arc<[_]> = (*p_fft_twiddles::<Felt>(a.0.len()).unwrap()).into();
        let pinned_ntt = PolyVec(p_ntt_twiddled(a.0, &twiddles));

        let inv_len = Felt::from_u64(binv(pinned_ntt.0.len() as _));
        let or = Belt(pinned_ntt.0.len() as _).ordered_root().unwrap();
        let root = Felt::from_u64(binv(or.0));
        let ifft_twiddles = (*p_ntt_twiddles::<Felt>(pinned_ntt.0.len(), &root)).into();

        Self {
            d,
            dq,
            deg_prod,
            pinned_ntt,
            inv_len,
            ifft_twiddles,
            twiddles,
        }
    }
}

#[tracing::instrument(skip_all)]
fn fpdiv_lead_rf(p: FPolyVec, WeightedDivConst { d, .. }: &WeightedDivConst) -> (Felt, FPolyVec) {
    assert!(!p.0.is_empty());

    let (c, f) = con_mon(p.clone());
    let lead = c / *d;
    let mut rf = f;
    rf.0.reverse();
    (lead, rf)
}

#[tracing::instrument(skip_all)]
fn fpdiv_with_cache<'a>(
    lead: Felt,
    rf: FPolyVec,
    WeightedDivConst {
        d: _,
        dq,
        pinned_ntt,
        deg_prod: _,
        inv_len,
        ifft_twiddles,
        twiddles,
    }: &WeightedDivConst,
) -> FPolyVec {
    let mulled = fpmul_fast_cached(pinned_ntt.into(), ifft_twiddles, twiddles, rf);
    let mut scagged = PolyVec(scag_vec(dq + 1, mulled.0));
    scagged.0.reverse();
    pscal_inplace(lead * *inv_len, &mut scagged.0);
    scagged
}

// ::  +fpmul-fast: polynomial multiplication with fft
fn fpmul_fast_cached<'a>(
    a: FPolySlice,
    ifft_twiddles: &[impl AsRef<[Felt]>],
    twiddles: &[impl AsRef<[Felt]>],
    b: FPolyVec,
) -> FPolyVec {
    let mut b = PolyVec(p_ntt_twiddled(b.0, &twiddles));
    p_hadamard_inplace(&mut b.0, &a.0);
    let ntt = p_ntt_twiddled(b.0, ifft_twiddles);
    PolyVec(ntt)
}

#[derive(Clone)]
pub struct LinearCombo {
    pub id_x: WeightedDivConst,
    pub lead_felts: Vec<Felt>,
    pub rf_polys: Vec<Felt>,
    pub weights_off: usize,
}

#[derive(Debug, Clone)]
pub struct DivisorBatch {
    pub polys: Vec<FPolyVec>,
    pub openings: Vec<Felt>,
    pub weights: Vec<Felt>,
    pub evaluation_point: Felt,
}

impl DivisorBatch {
    fn get_divisor_polynomial(&self) -> FPolyVec {
        let id = id_fpoly();
        let point_poly = new_fpoly(&[self.evaluation_point]);
        fpsub((&id).into(), (&point_poly).into())
    }

    #[tracing::instrument(skip_all)]
    fn weighted_division(self) -> FPolyVec {
        assert_eq!(self.polys.len(), self.openings.len());
        assert_eq!(self.polys.len(), self.weights.len());

        if self.polys.is_empty() {
            return zero_fpoly();
        }

        let div_const = WeightedDivConst::new(
            self.get_divisor_polynomial(),
            self.polys[0].0.len()
        );

        let mut weighted_numerator = zero_fpoly();

        for ((poly, opening), weight) in self.polys.iter().zip(self.openings).zip(self.weights) {
            let numerator = fpsub(poly.into(), PolySlice(&[opening]));
            let weighted_term = fpscal(weight, numerator);
            weighted_numerator = fpadd(weighted_numerator, (&weighted_term).into());
        }

        let (lead, mut rf) = fpdiv_lead_rf(weighted_numerator, &div_const);

        let expected_len = div_const.pinned_ntt.0.len();
        rf.0.resize(expected_len, Felt::zero());

        fpdiv_with_cache(lead, rf, &div_const)
    }
}

#[derive(Clone)]
pub struct DeepEngine<'a> {
    divisor_batches: Vec<DivisorBatch>,
    weights: FPolySlice<'a>,
}

impl<'a> DeepEngine<'a> {
    pub fn new(weights: FPolySlice<'a>) -> Self {
        Self {
            divisor_batches: vec![],
            weights,
        }
    }

    pub fn add_batch(&mut self, batch: DivisorBatch) {
        // It is in theory possible that multiple batches have the same point and can be combined.
        //  However, testing has shown this is not the case
        self.divisor_batches.push(batch);
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce(self) -> FPolyVec {
        self.divisor_batches
            .into_par_iter()
            .map(|batch| batch.weighted_division())
            .reduce(
                || zero_fpoly(),
                |acc, result| fpadd(acc, (&result).into())
            )
    }
}
