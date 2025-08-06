use std::rc::Rc;

use nbx_tip5::base::binv;
use nockvm::jets::JetErr;
use nockvm::mem::NockStack;
use zkvm_jetpack::form::math::poly::{
    p_fft_twiddles, p_hadamard_inplace, p_ntt_twiddled, p_ntt_twiddles, pscal_inplace,
};
use zkvm_jetpack::form::{Belt, ElementEx, FPolySlice, FPolyVec, Felt, PolySlice, PolyVec};

use crate::two::{
    con_mon, fdegree, fpadd, fpscal, fpsub, id_fpoly, pinv_mod_x_to, zero_fpoly, zeroextend_slice,
};
use crate::utils::{scag_vec, xeb};

#[derive(Clone)]
struct WeightedDivConst {
    d: Felt,
    dq: usize,
    deg_prod: usize,
    pinned_ntt: FPolyVec,
    twiddles: Rc<[Vec<Felt>]>,
    inv_len: Felt,
    ifft_twiddles: Rc<[Vec<Felt>]>,
}

impl WeightedDivConst {
    fn new(stack: &mut NockStack, q: FPolyVec, pl: usize) -> Self {
        let (d, g) = con_mon(q);
        let dg = fdegree((&g).into());

        let mut rg = g;
        rg.0.reverse();

        // fdegree(p)
        let df = pl - 1;

        let dq = df - dg;
        let pinned = pinv_mod_x_to(stack, dq + 1, (&rg).into());

        // =*  deg-p  len.fp
        let deg_p = pinned.0.len();
        // =*  deg-q  len.fq
        let deg_q = pl;

        // =/  deg-prod  (bex (xeb (dec (add deg-p deg-q))))
        let deg_prod = 1 << xeb(deg_p + deg_q - 1);

        let a = zeroextend_slice(pinned, deg_prod, Felt::zero());
        let twiddles = p_fft_twiddles::<Felt>(a.0.len()).unwrap();
        let pinned_ntt = PolyVec(p_ntt_twiddled(a.0, &twiddles));

        let inv_len = Felt::from_u64(binv(pinned_ntt.0.len() as _));
        let or = Belt(pinned_ntt.0.len() as _).ordered_root().unwrap();
        let root = Felt::from_u64(binv(or.0));
        let ifft_twiddles = p_ntt_twiddles::<Felt>(pinned_ntt.0.len(), &root);

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
struct LinearCombo {
    id_x: WeightedDivConst,
    preprocessed: Vec<(Felt, FPolyVec, Felt)>,
}

#[derive(Default, Clone)]
pub struct DeepEngine {
    combos: Vec<LinearCombo>,
}

impl DeepEngine {
    #[tracing::instrument(skip_all)]
    pub fn weighted_linear_combo<'a>(
        &mut self,
        stack: &mut NockStack,
        //cache: &mut HashMap<(FPolyVec, FPolyVec), core::result::Result<FPolyVec, JetErr>>,
        polys: &[FPolyVec],
        openings: FPolySlice<'a>,
        idx: usize,
        x_poly: FPolySlice<'a>,
        weights: FPolySlice<'a>,
    ) -> core::result::Result<usize, JetErr> {
        // |=  [polys=(list fpoly) openings=fpoly idx=@ x-poly=fpoly weights=fpoly]
        // ^-  [fpoly @]
        // =-  [acc num]
        let mut num = idx;

        let id = id_fpoly();
        let id_x = fpsub((&id).into(), x_poly);
        let id_x = WeightedDivConst::new(stack, id_x, polys[0].0.len());

        // %+  roll  polys
        // |=  [poly=fpoly acc=_zero-fpoly num=_idx]
        let mut preprocessed = vec![];
        for poly in polys {
            let poly: FPolySlice = poly.into();
            // :_  +(num)
            // %+  fpadd  acc
            // %+  fpscal  (~(snag fop weights) num)
            // %+  fpdiv
            //   (fpsub poly (fp-c (~(snag fop openings) num)))
            // (fpsub id-fpoly x-poly)
            // NOTE: id_x = (fpsub id-fpoly x-poly)
            let fpc = [openings.0[num]];
            /*println!(
            "id-x {} {} {}",
            vmug(stack, &id_x.0),
            vmug(stack, poly.0),
            vmug(stack, &fpc)
            );*/
            let fpc = PolySlice(&fpc);
            let r1 = fpsub(poly, fpc);

            let (lead, rf) = fpdiv_lead_rf(r1, &id_x);
            let rf = zeroextend_slice(rf, id_x.deg_prod, Felt::zero());
            preprocessed.push((lead, rf, weights.0[num]));

            num += 1;
        }

        self.combos.push(LinearCombo { id_x, preprocessed });

        Ok(num)
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce(self) -> FPolyVec {
        let mut acc: FPolyVec = zero_fpoly();

        for LinearCombo { id_x, preprocessed } in self.combos {
            for (lead, rf, weights) in preprocessed {
                let res = fpdiv_with_cache(lead, rf, &id_x);
                let res = fpscal(weights, res);
                acc = fpadd(acc, (&res).into());
            }
        }

        acc
    }
}
