use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::form::{ElementEx, FieldError};

#[inline]
fn bitreverse(mut n: u32, l: u32) -> u32 {
    let mut r = 0;
    for _ in 0..l {
        r = (r << 1) | (n & 1);
        n >>= 1;
    }
    r
}

#[inline(always)]
pub fn p_fft<T: ElementEx>(p: Vec<T>) -> Result<Vec<T>, FieldError> {
    let root = T::ordered_root(p.len() as u64)?;
    Ok(p_ntt(p, &root))
}

#[inline(always)]
pub fn p_fft_twiddles<T: ElementEx>(n: usize) -> Result<Rc<[Vec<T>]>, FieldError> {
    let root = T::ordered_root(n as u64)?;
    Ok(p_ntt_twiddles(n, &root))
}

struct TwiddleCache<T> {
    cache: BTreeMap<(usize, T), Rc<[Vec<T>]>>,
}

impl<T> Default for TwiddleCache<T> {
    fn default() -> Self {
        Self {
            cache: BTreeMap::new(),
        }
    }
}

impl<T: ElementEx> TwiddleCache<T> {
    fn twiddles(&mut self, n: usize, root: T) -> Rc<[Vec<T>]> {
        self.cache
            .entry((n, root))
            .or_insert_with(|| p_ntt_twiddles_impl(n, &root).into())
            .clone()
    }
}

#[derive(Default)]
struct TwiddleMultiCache {
    caches: RefCell<BTreeMap<TypeId, Box<dyn Any>>>,
}

impl TwiddleMultiCache {
    fn twiddles<T: ElementEx>(&self, n: usize, root: T) -> Rc<[Vec<T>]> {
        let mut caches = self.caches.borrow_mut();
        let c = caches.entry(TypeId::of::<T>()).or_insert_with(|| {
            let c = TwiddleCache::<T>::default();
            Box::new(c)
        });
        let c = c.downcast_mut::<TwiddleCache<T>>().unwrap();
        c.twiddles(n, root)
    }
}

thread_local! {
    static CACHE: TwiddleMultiCache = Default::default();
}

#[inline(never)]
pub fn p_ntt_twiddles<T: ElementEx>(n: usize, root: &T) -> Rc<[Vec<T>]> {
    CACHE.with(|c| c.twiddles(n, *root))
}

#[inline(never)]
pub fn p_ntt_twiddles_impl<T: ElementEx>(n: usize, root: &T) -> Vec<Vec<T>> {
    debug_assert!(n.is_power_of_two(), "n must be a power of two for NTT");
    let log_2_of_n = n.ilog2();

    let mut twiddle_factors = Vec::with_capacity(log_2_of_n as usize);
    let mut m_precompute = 1;
    for _stage_idx in 0..log_2_of_n {
        let w_m = root.epow((n / (2 * m_precompute)) as u64);
        let mut stage_twiddles = Vec::with_capacity(m_precompute as usize);
        let mut w = T::one();
        for _j in 0..m_precompute {
            stage_twiddles.push(w);
            w = w * w_m;
        }
        twiddle_factors.push(stage_twiddles);
        m_precompute *= 2;
    }

    twiddle_factors
}

#[inline(always)]
pub fn p_ntt<T: ElementEx>(p: Vec<T>, root: &T) -> Vec<T> {
    if p.len() == 1 {
        return p;
    }

    let twiddles = p_ntt_twiddles(p.len(), root);
    p_ntt_twiddled(p, &twiddles)
}

#[inline(never)]
pub fn p_ntt_twiddled<T: ElementEx>(mut x: Vec<T>, twiddles: &[impl AsRef<[T]>]) -> Vec<T> {
    let log_2_of_n = x.len().ilog2();

    for k in 0..x.len() {
        let rk = bitreverse(k as u32, log_2_of_n) as usize;
        if k < rk {
            x.swap(rk, k);
        }
    }

    for stage_idx in 0..log_2_of_n {
        let twiddles = twiddles[stage_idx as usize].as_ref();
        assert!(twiddles.len().is_power_of_two());
        for uv in x.chunks_exact_mut(2 * twiddles.len()) {
            let (u, v) = uv.split_at_mut(twiddles.len());
            for (w, (u_mut, v_mut)) in twiddles.iter().copied().zip(u.iter_mut().zip(v.iter_mut()))
            {
                let u = *u_mut;
                let v = *v_mut * w;
                *u_mut = u + v;
                *v_mut = u - v;
            }
        }
    }

    x
}
