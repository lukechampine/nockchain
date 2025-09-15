use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use const_for::const_for;

use super::binv;
use crate::form::{Belt, ElementEx, FieldError, Poly, PolySlice};

#[inline]
const fn bitreverse(mut n: u32, l: u32) -> u32 {
    let mut r = 0;
    const_for!(_ in 0..l => {
        r = (r << 1) | (n & 1);
        n >>= 1;
    });
    r
}

const fn generate_bit_reverse_table<const N: usize>() -> [u32; N] {
    let mut table = [0u32; N];
    let log_n = N.ilog2();

    let mut i = 0;
    while i < N {
        table[i] = bitreverse(i as u32, log_n);
        i += 1;
    }
    table
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

// ::  +fp-ifft: Inverse DFT with FFT algorithm
pub fn p_ifft<T: ElementEx>(p: Vec<T>) -> Result<Vec<T>, FieldError> {
    let inv_len = T::from_u64(binv(p.len() as _));
    let or = Belt(p.len() as _).ordered_root()?;
    let root = T::from_u64(binv(or.0));
    let mut ntt = p_ntt(p, &root);
    pscal_inplace(inv_len, &mut ntt);
    Ok(ntt)
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

#[inline(always)]
pub fn p_ntt_inplace<T: ElementEx>(p: &mut [T], root: &T) {
    if p.len() == 1 {
        return;
    }

    let twiddles = p_ntt_twiddles(p.len(), root);
    p_ntt_twiddled_inplace(p, &twiddles)
}

static BIT_REVERSE: [u32; 65536] = generate_bit_reverse_table::<65536>();
static BIT_REVERSE_4096: [u32; 4096] = generate_bit_reverse_table::<4096>();

#[inline(never)]
pub fn p_ntt_twiddled_inplace<T: ElementEx>(x: &mut [T], twiddles: &[impl AsRef<[T]>]) {
    debug_assert!(x.len() <= 65536);

    let Some(last_non_zero_index) = x.iter().rposition(|e| !e.is_zero()) else {
        // When all elements are zero, the twiddles won't change anything.
        return;
    };

    let log_2_of_n = x.len().ilog2();

    for k in 0..x.len() {
        // While the input size of this function is capped, it is also called with smaller inputs.
        //  The same lookup table can be re-used by truncating the bit reversal to the correct
        //  number of bits
        let rk = (BIT_REVERSE[k] >> (16 - log_2_of_n)) as usize;
        if k < rk {
            x.swap(rk, k);
        }
    }

    if x.len() == 65536 && last_non_zero_index < 1024 {
        p_ntt_twiddled_inplace_sparse(x, twiddles, log_2_of_n, last_non_zero_index, &BIT_REVERSE);
        return;
    }

    if x.len() == 4096 && last_non_zero_index < 512 {
        p_ntt_twiddled_inplace_sparse(
            x, twiddles, log_2_of_n, last_non_zero_index, &BIT_REVERSE_4096,
        );
        return;
    }

    p_ntt_twiddled_inplace_dense(x, twiddles, log_2_of_n);
}

#[inline(always)]
fn p_ntt_twiddled_inplace_dense<T: ElementEx>(
    x: &mut [T],
    twiddles: &[impl AsRef<[T]>],
    log_2_of_n: u32,
) {
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
}

#[inline(always)]
fn p_ntt_twiddled_inplace_sparse<T: ElementEx>(
    x: &mut [T],
    twiddles: &[impl AsRef<[T]>],
    log_2_of_n: u32,
    last_non_zero_index: usize,
    bit_reverse: &[u32],
) {
    // Twiddle operations spread out non-zero values throughout the vector. However, during the first
    //  iteration, this can be very sparse and a lot of operations can be skipped.
    let twiddles_stage0 = twiddles[0].as_ref();
    debug_assert_eq!(twiddles_stage0.len(), 1); // Should be [1]

    for i in 0..=last_non_zero_index {
        let i = bit_reverse[i] as usize;
        let j = i + 1;

        // Note that w = 1 for stage 0
        let u_val = x[i];
        let v_val = x[j];
        x[i] = u_val + v_val;
        x[j] = u_val - v_val;
    }

    // Process the remaining stages normally
    for stage_idx in 1..log_2_of_n {
        let twiddles_stage = twiddles[stage_idx as usize].as_ref();
        for uv in x.chunks_exact_mut(2 * twiddles_stage.len()) {
            let (u, v) = uv.split_at_mut(twiddles_stage.len());
            for (w, (u_mut, v_mut)) in twiddles_stage
                .iter()
                .copied()
                .zip(u.iter_mut().zip(v.iter_mut()))
            {
                let u_val = *u_mut;
                let v_val = *v_mut * w;
                *u_mut = u_val + v_val;
                *v_mut = u_val - v_val;
            }
        }
    }
}

#[inline(never)]
pub fn p_ntt_twiddled<T: ElementEx>(mut x: Vec<T>, twiddles: &[impl AsRef<[T]>]) -> Vec<T> {
    p_ntt_twiddled_inplace(&mut x, twiddles);
    x
}

pub fn padd<T: ElementEx>(a: &[T], b: &[T], res: &mut [T]) {
    let min: &[T];
    let max: &[T];
    if a.len() <= b.len() {
        min = a;
        max = b;
    } else {
        min = b;
        max = a;
    }

    for ((res_vec, max_vec), min_vec) in res
        .iter_mut()
        .zip(max)
        .zip(min.iter().map(Some).chain(std::iter::repeat(None)))
    {
        if let Some(min_vec) = min_vec {
            *res_vec = *min_vec + *max_vec;
        } else {
            *res_vec = *max_vec;
        }
    }
}

#[inline(always)]
pub fn padd_<T: ElementEx>(left: &[T], right: &[T]) -> Vec<T> {
    let len = std::cmp::max(left.len(), right.len());
    let mut res = vec![T::zero(); len];
    padd(left, right, res.as_mut_slice());
    res
}

#[inline(always)]
pub fn padd_in_place<T: ElementEx>(a: &mut [T], b: &[T]) {
    assert!(a.len() >= b.len());

    for (a_pelem, b_pelem) in a
        .iter_mut()
        .zip(b.iter().map(Some).chain(std::iter::repeat(None)))
    {
        if let Some(b_pelem) = b_pelem {
            *a_pelem = *b_pelem + *a_pelem;
        } else {
            break;
        }
    }
}

#[inline(always)]
pub fn psub<T: ElementEx>(a: &[T], b: &[T], res: &mut [T]) {
    let a_len = a.len();
    let b_len = b.len();

    let res_len = std::cmp::max(a_len, b_len);

    for i in 0..res_len {
        let n = i;
        if i < a_len && i < b_len {
            res[n] = a[n] - b[n];
        } else if i < a_len {
            res[n] = a[n];
        } else {
            res[n] = -b[n];
        }
    }
}

#[inline(always)]
pub fn psub_in_place<T: ElementEx>(a: &mut [T], b: &[T]) {
    debug_assert!(a.len() >= b.len());
    for (max_vec, min_vec) in a
        .iter_mut()
        .zip(b.iter().map(Some).chain(std::iter::repeat(None)))
    {
        if let Some(min_vec) = min_vec {
            *max_vec = *max_vec - *min_vec;
        } else {
            break;
        }
    }
    //  TODO: hoon impl does not normalize here, but maybe it should?
    //normalize_poly(a)
}

#[inline(always)]
pub fn psub_<T: ElementEx>(left: &[T], right: &[T]) -> Vec<T> {
    let len = std::cmp::max(left.len(), right.len());
    let mut res = vec![T::zero(); len];
    psub(left, right, res.as_mut_slice());
    res
}

#[inline(always)]
pub fn pmul<T: ElementEx>(a: &[T], b: &[T], res: &mut [T]) {
    if a.is_zero() || b.is_zero() {
        res.fill(T::zero());
        return;
    }

    res.fill(T::zero());

    let a_len = a.len();
    let b_len = b.len();

    for i in 0..a_len {
        if a[i].is_zero() {
            continue;
        }
        for j in 0..b_len {
            res[i + j] = res[i + j] + a[i] * b[j];
        }
    }
}

#[inline(always)]
pub fn pmul_<T: ElementEx>(left: &[T], right: &[T]) -> Vec<T> {
    let len = left.len() + right.len() - 1;
    let mut res = vec![T::zero(); len];
    pmul(left, right, res.as_mut_slice());
    res
}

#[inline(always)]
pub fn pscal_inplace<T: ElementEx, O: Into<T>>(scalar: O, b: &mut [T]) {
    let scalar = scalar.into();
    for bp in b.iter_mut() {
        *bp = scalar * *bp;
    }
}

#[inline(always)]
pub fn pscal<T: ElementEx>(scalar: T, b: &[T], res: &mut [T]) {
    for (res, bp) in res.iter_mut().zip(b.iter()) {
        *res = scalar * *bp;
    }
}

#[inline(always)]
pub fn pscal_<T: ElementEx>(scalar: T, b: &[T]) -> Vec<T> {
    let mut res = vec![T::zero(); b.len()];
    pscal(scalar, b, res.as_mut_slice());
    res
}

#[inline]
#[rustfmt::skip]
pub fn p_hadamard_inplace<T: ElementEx, O: Copy + Into<T>>(a: &mut [T], b: &[O]) {
    assert_eq!(
        a.len(),
        b.len(),
        "Unequal lengths: {}, {}",
        a.len(),
        b.len()
    );
    const CHUNK_SIZE: usize = 16;
    const PREFETCH_CHUNKS: usize = 4;
    let num_chunks = a.len() / CHUNK_SIZE;
    let mut a_chunks = a.array_chunks_mut::<CHUNK_SIZE>();
    let mut b_chunks = b.array_chunks::<CHUNK_SIZE>();

    for (i, (a, b)) in (&mut a_chunks).zip(&mut b_chunks).enumerate() {
        if i + PREFETCH_CHUNKS < num_chunks {
            unsafe {
                #[cfg(target_arch = "aarch64")]
                std::arch::asm!(
                    "prfm pldl1keep, [{0}]",
                    "prfm pldl1strm, [{1}]",
                    in(reg) a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS),
                    in(reg) b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS),
                    options(nostack, readonly)
                );

                #[cfg(target_arch = "x86_64")]
                {
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_ET0 }>(a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_ET0 }>(a.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS + CHUNK_SIZE / 2) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS) as *const _);
                    std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(b.as_ptr().add(CHUNK_SIZE * PREFETCH_CHUNKS + CHUNK_SIZE / 2) as *const _);
                }
            }
        }

        for (a, b) in a.iter_mut().zip(b) {
            *a *= (*b).into();
        }
    }

    for (a, b) in a_chunks
        .into_remainder()
        .iter_mut()
        .zip(b_chunks.remainder())
    {
        *a *= (*b).into();
    }
}

#[inline(always)]
pub fn p_hadamard<T: ElementEx>(a: &[T], b: &[T], res: &mut [T]) {
    assert_eq!(
        a.len(),
        b.len(),
        "Unequal lengths: {}, {}",
        a.len(),
        b.len()
    );
    res.iter_mut()
        .zip(a.iter())
        .zip(b.iter())
        .for_each(|((res_i, a_i), b_i)| {
            *res_i = *a_i * *b_i;
        });
}

#[inline(always)]
pub fn p_hadamard_<T: ElementEx>(a: &[T], b: &[T]) -> Vec<T> {
    assert_eq!(
        a.len(),
        b.len(),
        "Unequal lengths: {}, {}",
        a.len(),
        b.len()
    );
    let mut res = vec![T::zero(); a.len()];
    res.iter_mut()
        .zip(a.iter())
        .zip(b.iter())
        .for_each(|((res_i, a_i), b_i)| {
            *res_i = *a_i * *b_i;
        });
    res
}

#[inline(always)]
pub fn pneg<T: ElementEx>(b: &[T], res: &mut [T]) {
    for (res, bp) in res.iter_mut().zip(b.iter()) {
        *res = -*bp;
    }
}

#[inline(always)]
pub fn ppow<T: ElementEx>(a: &[T], mut n: usize) -> Vec<T> {
    let mut q = vec![T::one()];
    let mut p = a.to_vec();
    while n != 0 {
        if n & 1 == 1 {
            q = pmul_(&q, &p);
        } else {
            p = pmul_(&p, &p);
        }
        n >>= 1;
    }
    q
}

#[inline(always)]
pub fn p_shift<T: ElementEx>(poly_a: &[T], pelem_b: &T, poly_res: &mut [T]) {
    let mut pelem_power: T = T::one();

    for i in 0..poly_a.len() {
        poly_res[i] = poly_a[i] * pelem_power;
        pelem_power = pelem_power * *pelem_b;
    }

    for i in poly_a.len()..poly_res.len() {
        poly_res[i] = T::zero();
    }
}

#[inline(always)]
pub fn p_shift_nzero<T: ElementEx>(poly_a: &[T], pelem_b: &T, poly_res: &mut [T]) {
    let mut pelem_power: T = T::one();

    for i in 0..poly_a.len() {
        poly_res[i] = poly_a[i] * pelem_power;
        pelem_power = pelem_power * *pelem_b;
    }
}

#[inline(always)]
pub fn p_shift_inplace<T: ElementEx>(poly_a: &mut [T], pelem_b: &T) {
    let mut pelem_power: T = T::one();

    for p in poly_a {
        *p *= pelem_power;
        pelem_power = pelem_power * *pelem_b;
    }
}

#[inline(always)]
#[tracing::instrument(skip_all)]
pub fn p_coseword<T: ElementEx>(bp: &[T], offset: &T, order: u32, root: &T) -> Vec<T> {
    // shift
    let len_res: u32 = order;
    let mut res = vec![T::zero(); len_res as usize];

    if bp.is_zero() {
        return res;
    }

    p_shift_nzero(bp, offset, &mut res);

    p_ntt(res, root)
}

#[inline(always)]
#[tracing::instrument(skip_all)]
pub fn p_coseword_inplace<T: ElementEx>(bp: &[T], offset: &T, order: u32, root: &T, res: &mut [T]) {
    // shift
    let len_res: u32 = order;
    assert_eq!(len_res as usize, res.len());
    p_shift(bp, offset, res);

    p_ntt_inplace(res, root);
}

#[inline(always)]
pub fn poly_zero_extend<T: ElementEx>(a: &[T], res: &mut [T]) {
    let a_len = a.len();
    let res_len = res.len();
    res[0..a_len].copy_from_slice(a);
    res[a_len..res_len].fill(T::zero());
}

pub fn pcan<T: ElementEx>(mut p: Vec<T>) -> Vec<T> {
    while let Some(v) = p.last() {
        if v.is_zero() {
            p.pop();
        } else {
            break;
        }
    }
    if p.is_empty() {
        p.push(T::zero());
    }
    p
}

// TODO: make res return itself as Vec
#[inline(always)]
pub fn pdvr<T: ElementEx>(a: &[T], b: &[T], q: &mut [T], res: &mut [T]) {
    if a.is_zero() {
        q.fill(T::zero());
        res.fill(T::zero());
        return;
    }

    assert!(!b.is_zero(), "Division by zero polynomial");

    q.fill(T::zero());
    res.fill(T::zero());

    let deg_a = a.degree();
    let deg_b = b.degree();

    let a_end = deg_a as usize;
    let mut r = a[0..(a_end + 1)].to_vec();

    // Pre-compute the inverse of leading coefficient of b to avoid repeated divisions
    let b_lead_inv = T::one() / b[deg_b as usize];

    let mut i = a_end;
    let end_b = deg_b as usize;
    let mut deg_r = deg_a;
    let mut q_index = deg_r.saturating_sub(deg_b);

    while deg_r >= deg_b {
        let coeff = r[i] * b_lead_inv;
        q[q_index as usize] = coeff;
        for k in 0..(deg_b + 1) {
            let index = k as usize;
            if k <= a_end as u32 && k < b.len() as u32 && k <= (i as u32) {
                r[i - index] = r[i - index] - coeff * b[end_b - index];
            }
        }
        deg_r = deg_r.saturating_sub(1);
        q_index = q_index.saturating_sub(1);
        if deg_r == 0 && r[0] == T::zero() {
            break;
        }
        i -= 1;
    }

    let r_len = deg_r + 1;
    res[0..(r_len as usize)].copy_from_slice(&r[0..(r_len as usize)]);
}

#[inline(always)]
pub fn pdvr_vec<T: ElementEx>(a: &[T], b: &[T]) -> (Vec<T>, Vec<T>) {
    let deg_a = a.degree();
    let deg_b = b.degree();
    let deg_q = deg_a.saturating_sub(deg_b);
    let len_q = deg_q + 1;
    let len_r = deg_b + 1;

    let mut q = vec![T::zero(); len_q as usize];
    let mut r = vec![T::zero(); len_r as usize];

    pdvr(a, b, q.as_mut_slice(), r.as_mut_slice());

    (q, r)
}

#[inline(always)]
pub fn pdiv<T: ElementEx>(a: &[T], b: &[T]) -> Vec<T> {
    pdvr_vec(&a, &b).0
}

// ::
// ::  +bp-decompose
// ::
// ::  given a polynomial f(X) of degree at most D*N, decompose into D polynomials
// ::  {h_i(X) : 0 <= i < D} each of degree at most N such that
// ::
// ::  f(X) = h_0(X^D) + X*h_1(X^D) + X^2*h_2(X^D) + ... + X^{D-1}*h_{D-1}(X^D)
// ::
// ::  This is just a generalization of splitting a polynomial into even and odd terms
// ::  as the FFT does.
// ::  h_i(X) is the terms whose degree is congruent to i modulo D.
// ::
// ::  Passing in d=2 will split into even and odd terms.
// ::
pub fn p_decompose<T: ElementEx>(p: PolySlice<T>, d: usize) -> Vec<Vec<T>> {
    // |=  [p=bpoly d=@]
    // ^-  (list bpoly)
    // =/  total-deg=@  (bdegree (bpoly-to-list p))
    let total_deg = p.degree() as usize;
    // =/  deg=@
    //   =/  dvr  (dvr total-deg d)
    let dvr_p = total_deg / d;
    let dvr_q = total_deg % d;
    //   ?:(=(q.dvr 0) p.dvr (add p.dvr 1))
    let deg = dvr_p + (dvr_q != 0) as usize;
    // =/  acc=(list (list belt))  (reap d ~)
    let mut acc = vec![vec![]; d];
    // =-
    //   %+  turn  -
    //   |=  poly=(list belt)
    //   ?~  poly  zero-bpoly
    //   (init-bpoly (flop poly))
    // %+  roll  (range (add 1 deg))
    // |=  [n=@ acc=_acc]
    for n in 0..=deg {
        // %+  iturn  acc
        // |=  [i=@ l=(list belt)]
        for i in 0..d {
            // =/  idx  (add (mul n d) i)
            let idx = n * d + i;
            // ?:  (gth idx total-deg)  l
            if idx <= total_deg {
                // [(~(snag bop p) idx) l]
                acc[i].push(p.0[idx]);
            }
        }
    }
    acc
}

// ::  fpeval: evaluate a polynomial with Horner's method.
pub fn peval<T: ElementEx>(p: PolySlice<T>, x: T) -> T {
    // |:  [fp=`fpoly`one-fpoly x=`felt`(lift 1)]
    // ^-  felt
    // ~+
    // ?:  (fp-is-zero fp)  (lift 0)
    if p.is_zero() {
        return T::from_u64(0);
    }
    // ?:  =(len.fp 1)  (~(snag fop fp) 0)
    if p.len() == 1 {
        return p.0[0];
    }
    // =/  p  ~(to-poly fop fp)
    // =.  p  (flop p)
    // =/  res=@  (lift 0)
    let mut res = T::zero();

    // |-
    // ?~  p    !!
    // ?~  t.p
    //   (fadd (fmul res x) i.p)
    // ::  based on p(x) = (...((a_n)x + a_{n-1})x + a_{n-2})x + ... )
    // $(res (fadd (fmul res x) i.p), p t.p)
    for p in p.0.iter().rev() {
        res = res * x + *p;
    }

    res
}
