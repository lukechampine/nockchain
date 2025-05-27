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

pub fn p_ntt<T: ElementEx>(p: Vec<T>, root: &T) -> Vec<T> {
    let n = p.len() as u32;

    if n == 1 {
        return p;
    }

    debug_assert!(n.is_power_of_two());

    let log_2_of_n = n.ilog2();

    let mut x = p;

    for k in 0..n {
        let rk = bitreverse(k, log_2_of_n);
        if k < rk {
            x.swap(rk as usize, k as usize);
        }
    }

    let mut m = 1;
    for _ in 0..log_2_of_n {
        let w_m: T = root.epow((n / (2 * m)) as u64);

        let mut k = 0;
        while k < n {
            let mut w = T::one();

            for j in 0..m {
                let u: T = x[(k + j) as usize];
                let v: T = x[(k + j + m) as usize] * w;
                x[(k + j) as usize] = u + v;
                x[(k + j + m) as usize] = u - v;
                w = w * w_m;
            }

            k += 2 * m;
        }

        m *= 2;
    }
    x
}
