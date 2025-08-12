use std::rc::Rc;
use crate::log::*;

use nbx_tip5::base::binv;
use nockvm::jets::JetErr;
use nockvm::mem::NockStack;
use zkvm_jetpack::form::math::poly::{
    p_fft_twiddles, p_hadamard_inplace, p_ntt_twiddled, p_ntt_twiddles, pscal_inplace,
};
use zkvm_jetpack::form::{Belt, ElementEx, FPolySlice, FPolyVec, Felt, PolySlice, PolyVec};

#[cfg(feature = "gpu")]
use super::gpu;
use crate::two::{
    con_mon, fdegree, fpadd, fpscal, fpsub, id_fpoly, pinv_mod_x_to, zero_fpoly, zeroextend_slice,
};
use crate::utils::{scag_vec, xeb};

#[derive(Clone)]
pub struct WeightedDivConst {
    pub d: Felt,
    pub dq: usize,
    pub deg_prod: usize,
    pub pinned_ntt: FPolyVec,
    pub twiddles: Rc<[Vec<Felt>]>,
    pub inv_len: Felt,
    pub ifft_twiddles: Rc<[Vec<Felt>]>,
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
pub struct LinearCombo {
    pub id_x: WeightedDivConst,
    pub lead_felts: Vec<Felt>,
    pub rf_polys: Vec<Felt>,
    pub weights_off: usize,
}

#[derive(Clone)]
pub struct DeepEngine<'a> {
    combos: Vec<LinearCombo>,
    weights: FPolySlice<'a>,
}

impl<'a> DeepEngine<'a> {
    pub fn new(weights: FPolySlice<'a>) -> Self {
        Self {
            combos: vec![],
            weights,
        }
    }

    pub fn destruct(self) -> (Vec<LinearCombo>, FPolySlice<'a>) {
        (self.combos, self.weights)
    }

    #[tracing::instrument(skip_all)]
    pub fn weighted_linear_combo(
        &mut self,
        //stack: &mut NockStack,
        //cache: &mut HashMap<(FPolyVec, FPolyVec), core::result::Result<FPolyVec, JetErr>>,
        polys: &[FPolyVec],
        openings: FPolySlice,
        idx: usize,
        x_poly: FPolySlice,
        mut idx_poly: usize,
    ) -> core::result::Result<usize, JetErr> {
        // |=  [polys=(list fpoly) openings=fpoly idx=@ x-poly=fpoly weights=fpoly]
        // ^-  [fpoly @]
        // =-  [acc num]
        let mut num = idx;
        let weights_off = num;

        let id = id_fpoly();
        let id_x = fpsub((&id).into(), x_poly);
        let id_x = WeightedDivConst::new(id_x, polys[0].0.len());

        // %+  roll  polys
        // |=  [poly=fpoly acc=_zero-fpoly num=_idx]
        let mut rf_polys = Vec::with_capacity(polys.len() * id_x.deg_prod);
        let mut lead_felts = Vec::with_capacity(polys.len());

        for poly in polys {
            let poly: FPolySlice = poly.into();
            // :_  +(num)
            // %+  fpadd  acc
            // %+  fpscal  (~(snag fop weights) num)
            // %+  fpdiv
            //   (fpsub poly (fp-c (~(snag fop openings) num)))
            // (fpsub id-fpoly x-poly)
            // NOTE: id_x = (fpsub id-fpoly x-poly)
            let fpc = [openings.0[idx_poly]];
            /*println!(
            "id-x {} {} {}",
            vmug(stack, &id_x.0),
            vmug(stack, poly.0),
            vmug(stack, &fpc)
            );*/
            let fpc = PolySlice(&fpc);
            let r1 = fpsub(poly, fpc);

            let (lead, rf) = fpdiv_lead_rf(r1, &id_x);
            rf_polys.extend_from_slice(&rf.0);
            rf_polys.resize(rf_polys.len() + id_x.deg_prod - rf.0.len(), Felt::zero());
            lead_felts.push(lead);

            num += 1;
            idx_poly += 1;
        }

        self.combos.push(LinearCombo {
            id_x,
            rf_polys,
            lead_felts,
            weights_off,
        });

        Ok(num)
    }

    pub fn reduce(self) -> FPolyVec {
        // If the GPU is not enabled, return the CPU result directly
        #[cfg(not(feature = "gpu"))]
        return self.reduce_cpu();

        #[cfg(feature = "gpu")]
        {
            // If the GPU is enabled but should not be used, return the CPU result directly
            if !gpu::should_use_gpu() {
                return self.reduce_cpu();
            }

            // If the GPU result is not validated, return the GPU result directly
            #[cfg(not(feature = "validate-gpu"))]
            return self.reduce_gpu();

            #[cfg(feature = "validate-gpu")]
            {
                let cpu_result = self.clone().reduce_cpu();
                let gpu_result = self.reduce_gpu();

                // Emit warnings if the CPU and GPU results are different
                if cpu_result != gpu_result {
                    warn!("The result of the CPU and GPU are different for the `deep` operation")
                }

                // Even if the result is computed using a GPU for validation, always return the CPU
                //  result as it is considered more reliable.
                cpu_result
            }
        }
    }

    #[cfg(feature = "gpu")]
    #[tracing::instrument(skip_all)]
    pub fn reduce_gpu(self) -> FPolyVec {
        use super::gpu::Submittable;
        Submittable::gpu_process(self).res
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce_cpu(self) -> FPolyVec {
        let mut acc: FPolyVec = zero_fpoly();

        for LinearCombo {
            id_x,
            rf_polys,
            lead_felts,
            weights_off,
        } in self.combos
        {
            let rf = rf_polys.chunks_exact(id_x.deg_prod);
            let mut rf_vec = Vec::with_capacity(id_x.deg_prod);
            let weights = &self.weights.0[weights_off..];
            for ((lead, rf), weights) in lead_felts.into_iter().zip(rf).zip(weights) {
                rf_vec.clear();
                rf_vec.extend_from_slice(rf);
                let res = fpdiv_with_cache(lead, PolyVec(rf_vec), &id_x);
                let res = fpscal(*weights, res);
                acc = fpadd(acc, (&res).into());
                rf_vec = res.0;
            }
        }

        acc
    }
}
