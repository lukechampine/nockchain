use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nockchain_math::belt::Belt;

fn create_sparse_input(size: usize, last_non_zero_index: usize) -> Vec<Belt> {
    (0..last_non_zero_index)
        .map(|_| Belt(rand::random()))
        .chain((last_non_zero_index..size).map(|_| Belt::zero()))
        .collect()
}

fn bench_p_coseword(c: &mut Criterion) {
    let mut group = c.benchmark_group("p_coseword");

    // Test with different orders/sizes
    for (order, bp_len) in [(65536, 32768)] {
        let root = Belt(17492915097719143606);

        group.bench_function(
            BenchmarkId::new("p_coseword", format!("order_{}_bp_len_{}", order, bp_len)),
            |b| {
                b.iter_with_setup(
                    || {
                        // Create input polynomial with some non-zero elements
                        let bp: Vec<Belt> = (0..bp_len).map(|_| Belt(rand::random())).collect();
                        let offset = Belt(rand::random());
                        (bp, offset)
                    },
                    |(bp, offset)| {
                        nockchain_math::poly_ext::p_coseword(
                            black_box(&bp),
                            black_box(&offset),
                            black_box(order),
                            black_box(&root),
                        );
                    },
                );
            },
        );

        // Also test sparse case (mostly zeros)
        group.bench_function(
            BenchmarkId::new(
                "p_coseword_sparse",
                format!("order_{}_bp_len_{}", order, bp_len),
            ),
            |b| {
                b.iter_with_setup(
                    || {
                        let last_non_zero = bp_len / 64; // Very sparse
                        let bp: Vec<Belt> = create_sparse_input(bp_len, last_non_zero);
                        let offset = Belt(rand::random());
                        (bp, offset)
                    },
                    |(bp, offset)| {
                        nockchain_math::poly_ext::p_coseword(
                            black_box(&bp),
                            black_box(&offset),
                            black_box(order),
                            black_box(&root),
                        );
                    },
                );
            },
        );

        group.bench_function(BenchmarkId::new("p_coseword_zero", order), |b| {
            b.iter_with_setup(
                || {
                    let bp = vec![Belt::zero(); bp_len];
                    let offset = Belt(rand::random());
                    (bp, offset)
                },
                |(bp, offset)| {
                    nockchain_math::poly_ext::p_coseword(
                        black_box(&bp),
                        black_box(&offset),
                        black_box(order),
                        black_box(&root),
                    );
                },
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_p_coseword);
criterion_main!(benches);
