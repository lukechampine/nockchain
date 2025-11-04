use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nockchain_math::belt::Belt;
use nockchain_math::poly_ext::{p_ntt_twiddles, BIT_REVERSE};

fn create_sparse_input(size: usize, last_non_zero_index: usize) -> Vec<Belt> {
    (0..last_non_zero_index)
        .map(|_| Belt(rand::random()))
        .chain((last_non_zero_index..size).map(|_| Belt::zero()))
        .collect()
}

fn create_sparse_test_data(size: usize, last_non_zero_index: usize) -> (u32, Vec<Belt>) {
    let mut data = create_sparse_input(size, last_non_zero_index);

    let log_2_of_n = size.ilog2();

    for k in 0..size {
        // While the input size of this function is capped, it is also called with smaller inputs.
        //  The same lookup table can be re-used by truncating the bit reversal to the correct
        //  number of bits
        let rk = (BIT_REVERSE[k] >> (16 - log_2_of_n)) as usize;
        if k < rk {
            data.swap(rk, k);
        }
    }

    (log_2_of_n, data)
}

fn bench_p_ntt(c: &mut Criterion) {
    let mut group = c.benchmark_group("p_ntt");

    let twiddles = p_ntt_twiddles(65536, &Belt(17492915097719143606));

    group.bench_function(BenchmarkId::new("p_ntt_twiddled_inplace", "65536"), |b| {
        b.iter_with_setup(
            || create_sparse_input(65536, 1023),
            |mut input| {
                nockchain_math::poly_ext::p_ntt_twiddled_inplace(
                    black_box(&mut input),
                    black_box(&twiddles),
                );
            },
        );
    });

    group.bench_function(BenchmarkId::new("dense", "65536"), |b| {
        b.iter_with_setup(
            || create_sparse_test_data(65536, 1023),
            |(log_2_of_n, mut input)| {
                nockchain_math::p_ntt::scalar::dense(
                    black_box(&mut input),
                    black_box(&twiddles),
                    black_box(log_2_of_n),
                );
            },
        );
    });

    group.bench_function(BenchmarkId::new("sparse", "65536"), |b| {
        b.iter_with_setup(
            || create_sparse_test_data(65536, 1023),
            |(log_2_of_n, mut input)| {
                nockchain_math::p_ntt::scalar::sparse(
                    black_box(&mut input),
                    black_box(&twiddles),
                    black_box(log_2_of_n),
                    black_box(1023),
                    black_box(&BIT_REVERSE),
                );
            },
        );
    });
}

criterion_group!(benches, bench_p_ntt);
criterion_main!(benches);
