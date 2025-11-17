use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(target_arch = "x86_64")]
use nbx_tip5::tip5::avx512;
use nbx_tip5::tip5::test_cases::{
    get_fixed_instance, get_fixed_instances_x2, get_fixed_instances_x8, get_instance,
    get_instances_x2, get_instances_x8, get_instances_x8_split,
};
use nbx_tip5::tip5::{scalar, simd, simd_x2, simd_x8};

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut counter = 0;
    c.bench_function("permute", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |mut input_copy| nbx_tip5::tip5::permute(std::hint::black_box(&mut input_copy)),
        )
    });
    c.bench_function("permute scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |mut input_copy| scalar::permute(std::hint::black_box(&mut input_copy)),
        )
    });
    c.bench_function("permute simd", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |mut input_copy| simd::permute(std::hint::black_box(&mut input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    get_instance(counter)
                },
                |mut input_copy| unsafe { avx512::permute(std::hint::black_box(&mut input_copy)) },
            )
        });
    }

    // FIXED
    c.bench_function("permute fixed scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_fixed_instance(counter)
            },
            |input_copy| scalar::permute_fixed(&std::hint::black_box(input_copy)),
        )
    });
    c.bench_function("permute fixed simd", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_fixed_instance(counter)
            },
            |input_copy| simd::permute_fixed(&std::hint::black_box(input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute fixed avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    get_fixed_instance(counter)
                },
                |input_copy| unsafe { avx512::permute_fixed(std::hint::black_box(&input_copy)) },
            )
        });
    }

    c.bench_function("permute_fixed_x2 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_fixed_instances_x2(counter)
            },
            |(a, b)| {
                scalar::permute_fixed(std::hint::black_box(&a));
                scalar::permute_fixed(std::hint::black_box(&b));
            },
        )
    });
    c.bench_function("permute_fixed_x2 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_fixed_instances_x2(counter)
            },
            |(a, b)| simd_x2::permute_fixed_x2(std::hint::black_box(&a), std::hint::black_box(&b)),
        )
    });

    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute_fixed_x2 avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 2;
                    get_fixed_instances_x2(counter)
                },
                |(a, b)| unsafe {
                    avx512::permute_fixed_x2(std::hint::black_box(&a), std::hint::black_box(&b))
                },
            )
        });
    }

    c.bench_function("permute_fixed_x8 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_fixed_instances_x8(counter)
            },
            |instances| {
                scalar::permute_fixed(std::hint::black_box(&instances[0]));
                scalar::permute_fixed(std::hint::black_box(&instances[1]));
                scalar::permute_fixed(std::hint::black_box(&instances[2]));
                scalar::permute_fixed(std::hint::black_box(&instances[3]));
                scalar::permute_fixed(std::hint::black_box(&instances[4]));
                scalar::permute_fixed(std::hint::black_box(&instances[5]));
                scalar::permute_fixed(std::hint::black_box(&instances[6]));
                scalar::permute_fixed(std::hint::black_box(&instances[7]));
            },
        )
    });
    c.bench_function("permute_fixed_x8 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_fixed_instances_x8(counter)
            },
            |instances| simd_x8::permute_fixed_x8(std::hint::black_box(instances)),
        )
    });

    // INTERMEDIATE
    c.bench_function("permute intermediate scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |mut input_copy| scalar::permute_intermediate(std::hint::black_box(&mut input_copy)),
        )
    });
    c.bench_function("permute intermediate simd", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |mut input_copy| simd::permute_intermediate(std::hint::black_box(&mut input_copy)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute intermediate avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 1;
                    get_instance(counter)
                },
                |mut input_copy| unsafe {
                    avx512::permute_intermediate(std::hint::black_box(&mut input_copy))
                },
            )
        });
    }

    c.bench_function("permute_intermediate_x2 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_instances_x2(counter)
            },
            |(mut a, mut b)| {
                scalar::permute_intermediate(std::hint::black_box(&mut a));
                scalar::permute_intermediate(std::hint::black_box(&mut b));
            },
        )
    });
    c.bench_function("permute_intermediate_x2 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_instances_x2(counter)
            },
            |(mut a, mut b)| {
                simd_x2::permute_intermediate_x2(
                    std::hint::black_box(&mut a),
                    std::hint::black_box(&mut b),
                );
            },
        )
    });

    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute_intermediate_x2 avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 2;
                    get_instances_x2(counter)
                },
                |(mut a, mut b)| unsafe {
                    avx512::permute_intermediate_x2(
                        std::hint::black_box(&mut a),
                        std::hint::black_box(&mut b),
                    )
                },
            )
        });
    }

    c.bench_function("permute_intermediate_x8 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_instances_x8(counter)
            },
            |mut instances| {
                scalar::permute_intermediate(std::hint::black_box(&mut instances[0]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[1]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[2]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[3]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[4]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[5]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[6]));
                scalar::permute_intermediate(std::hint::black_box(&mut instances[7]));
            },
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute_intermediate_x8 avx512 4x2", |b| {
            b.iter_with_setup(
                || {
                    counter += 8;
                    get_instances_x8_split(counter)
                },
                |(mut a, mut b)| unsafe {
                    avx512::permute_intermediate_x2(
                        std::hint::black_box(&mut a[0]),
                        std::hint::black_box(&mut b[0]),
                    );
                    avx512::permute_intermediate_x2(
                        std::hint::black_box(&mut a[1]),
                        std::hint::black_box(&mut b[1]),
                    );
                    avx512::permute_intermediate_x2(
                        std::hint::black_box(&mut a[2]),
                        std::hint::black_box(&mut b[2]),
                    );
                    avx512::permute_intermediate_x2(
                        std::hint::black_box(&mut a[3]),
                        std::hint::black_box(&mut b[3]),
                    );
                },
            )
        });
    }
    c.bench_function("permute_intermediate_x8 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_instances_x8(counter)
            },
            |mut instances| {
                simd_x8::permute_intermediate_x8(std::hint::black_box(&mut instances));
            },
        )
    });

    // LAST
    c.bench_function("permute last scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
            },
            |input_copy| scalar::permute_last(std::hint::black_box(input_copy)),
        )
    });
    c.bench_function("permute last simd", |b| {
        b.iter_with_setup(
            || {
                counter += 1;
                get_instance(counter)
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
                    get_instance(counter)
                },
                |input_copy| unsafe { avx512::permute_last(std::hint::black_box(input_copy)) },
            )
        });
    }

    c.bench_function("permute_last_x2 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_instances_x2(counter)
            },
            |(a, b)| {
                scalar::permute_last(std::hint::black_box(a));
                scalar::permute_last(std::hint::black_box(b));
            },
        )
    });
    c.bench_function("permute_last_x2 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 2;
                get_instances_x2(counter)
            },
            |(a, b)| simd_x2::permute_last_x2(std::hint::black_box(a), std::hint::black_box(b)),
        )
    });
    #[cfg(target_arch = "x86_64")]
    if avx512::cpu_supported() {
        c.bench_function("permute_last_x2 avx512", |b| {
            b.iter_with_setup(
                || {
                    counter += 2;
                    get_instances_x2(counter)
                },
                |(a, b)| unsafe {
                    avx512::permute_last_x2(std::hint::black_box(a), std::hint::black_box(b))
                },
            )
        });
    }

    c.bench_function("permute_last_x8 scalar", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_instances_x8(counter)
            },
            |instances| {
                scalar::permute_last(std::hint::black_box(instances[0]));
                scalar::permute_last(std::hint::black_box(instances[1]));
                scalar::permute_last(std::hint::black_box(instances[2]));
                scalar::permute_last(std::hint::black_box(instances[3]));
                scalar::permute_last(std::hint::black_box(instances[4]));
                scalar::permute_last(std::hint::black_box(instances[5]));
                scalar::permute_last(std::hint::black_box(instances[6]));
                scalar::permute_last(std::hint::black_box(instances[7]));
            },
        )
    });
    c.bench_function("permute_last_x8 simd", |b| {
        b.iter_with_setup(
            || {
                counter += 8;
                get_instances_x8(counter)
            },
            |instances| simd_x8::permute_last_x8(std::hint::black_box(instances)),
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
