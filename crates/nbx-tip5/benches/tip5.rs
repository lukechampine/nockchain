use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(target_arch = "x86_64")]
use nbx_tip5::tip5::avx512;
use nbx_tip5::tip5::scalar;
use nbx_tip5::tip5::test_cases::INSTANCES;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut counter = 0;
    c.bench_function("permute", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                INSTANCES[counter % INSTANCES.len()]
            },
            |mut input_copy| nbx_tip5::tip5::permute(std::hint::black_box(&mut input_copy)),
        )
    });
    c.bench_function("permute_x2", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                (
                    INSTANCES[counter % INSTANCES.len()][..10]
                        .try_into()
                        .unwrap(),
                    INSTANCES[(counter + 1) % INSTANCES.len()][..10]
                        .try_into()
                        .unwrap(),
                )
            },
            |(a, b)| {
                nbx_tip5::tip5::permute_fixed_x2(std::hint::black_box(a), std::hint::black_box(b))
            },
        )
    });
    c.bench_function("permute scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                INSTANCES[counter % INSTANCES.len()]
            },
            |mut input_copy| scalar::permute(std::hint::black_box(&mut input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    INSTANCES[counter % INSTANCES.len()]
                },
                |mut input_copy| unsafe { avx512::permute(std::hint::black_box(&mut input_copy)) },
            )
        });
    }
    c.bench_function("permute intermediate scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                INSTANCES[counter % INSTANCES.len()]
            },
            |mut input_copy| scalar::permute_intermediate(std::hint::black_box(&mut input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute intermediate avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    INSTANCES[counter % INSTANCES.len()]
                },
                |mut input_copy| unsafe {
                    avx512::permute_intermediate(std::hint::black_box(&mut input_copy))
                },
            )
        });
    }
    c.bench_function("permute last scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                INSTANCES[counter % INSTANCES.len()]
            },
            |input_copy| scalar::permute_last(std::hint::black_box(input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute last avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    INSTANCES[counter % INSTANCES.len()]
                },
                |input_copy| unsafe { avx512::permute_last(std::hint::black_box(input_copy)) },
            )
        });
    }
    c.bench_function("permute fixed scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                INSTANCES[counter % INSTANCES.len()][..10]
                    .try_into()
                    .unwrap()
            },
            |input_copy| scalar::permute_fixed(&std::hint::black_box(input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute fixed avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    INSTANCES[counter % INSTANCES.len()][..10]
                        .try_into()
                        .unwrap()
                },
                |input_copy| unsafe { avx512::permute_fixed(std::hint::black_box(input_copy)) },
            )
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
