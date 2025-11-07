use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nbx_tip5::melt::Melt;
use nockchain_math::hadamard;

fn bench_hadamard(c: &mut Criterion) {
    let mut group = c.benchmark_group("hadamard");

    let size = 4096;
    let mut a: Vec<Melt> = (0..size).map(|i| Melt(i as u64)).collect();
    let b: Vec<Melt> = (0..size).map(|i| Melt((i * 2) as u64)).collect();

    group.bench_function("4096", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::p_hadamard_inplace(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.bench_function("4096 scalar", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::scalar::p_hadamard_inplace(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.bench_function("4096 scalar chunked", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::scalar::p_hadamard_inplace_chunked(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.bench_function("4096 scalar prefetched", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::scalar::p_hadamard_inplace_chunked(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.bench_function("4096 simd x2", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::simd::p_hadamard_inplace_4096_melt_x2(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.bench_function("4096 simd x8", |bencher| {
        bencher.iter(|| {
            let mut a_copy = a.clone();
            hadamard::simd::p_hadamard_inplace_4096_melt_x8(black_box(&mut a_copy), black_box(&b));
            black_box(a_copy);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_hadamard);
criterion_main!(benches);
