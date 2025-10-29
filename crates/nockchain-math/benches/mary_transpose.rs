use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nockchain_math::mary::{MarySlice, MarySliceMut};

fn create_test_data(rows: usize, cols: usize) -> Vec<u64> {
    (0..rows * cols).map(|i| i as u64).collect()
}

fn bench_transpose(c: &mut Criterion) {
    let sizes = vec![
        (195, 65536, "195x65536"),
    ];

    for (rows, cols, name) in sizes {
        let src = create_test_data(rows, cols);

        let mut group = c.benchmark_group(format!("transpose_{}", name));

        group.bench_function(BenchmarkId::new("original", name), |b| {
            b.iter_with_setup(
                || {
                    let mut dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose(
                        black_box(fpolys),
                        black_box(1),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("offset 1", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("blocked_64", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1_blocked::<64>(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("blocked_32", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1_blocked::<32>(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("blocked_16", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1_blocked::<16>(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("blocked_8", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1_blocked::<16>(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        group.bench_function(BenchmarkId::new("ptr", name), |b| {
            b.iter_with_setup(
                || {
                    let dst = vec![0u64; rows * cols];

                    let fpolys = MarySlice {
                        dat: &src,
                        len: rows as u32,
                        step: cols as u32,
                    };

                    (fpolys, dst)
                },
                |(fpolys, mut dst)| {
                    let mut res = MarySliceMut {
                        dat: &mut dst,
                        len: cols as u32,
                        step: rows as u32,
                    };

                    nockchain_math::mary_transpose::scalar::mary_transpose_offset_1_ptr(
                        black_box(fpolys),
                        black_box(&mut res),
                    );
                },
            );
        });

        // NEON optimisations
        #[cfg(target_arch = "aarch64")]
        {
            group.bench_function(BenchmarkId::new("neon 2x2", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::neon::mary_transpose_offset_1_2x2(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("neon blocked 32 2x2", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::neon::mary_transpose_offset_1_blocked_2x2::<32>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("neon blocked 32 4x2", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::neon::mary_transpose_offset_1_blocked_4x2::<32>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("neon blocked 64 4x4", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::neon::mary_transpose_offset_1_blocked_4x4::<64>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });
        }

        // AVX512 optimizations
        #[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
        {
            group.bench_function(BenchmarkId::new("avx512 offset 1", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_8x8(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("avx512 offset 1 - nt", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_8x8_nt(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("avx512 blocked 128", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8::<128>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("avx512 blocked 64", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8::<64>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });



            group.bench_function(BenchmarkId::new("avx512 blocked 32", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8::<32>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("avx512 blocked 128 - nt", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8_nt::<128>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });

            group.bench_function(BenchmarkId::new("avx512 blocked 64 - nt", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8_nt::<64>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });



            group.bench_function(BenchmarkId::new("avx512 blocked 32 - nt", name), |b| {
                b.iter_with_setup(
                    || {
                        let dst = vec![0u64; rows * cols];

                        let fpolys = MarySlice {
                            dat: &src,
                            len: rows as u32,
                            step: cols as u32,
                        };

                        (fpolys, dst)
                    },
                    |(fpolys, mut dst)| unsafe {
                        let mut res = MarySliceMut {
                            dat: &mut dst,
                            len: cols as u32,
                            step: rows as u32,
                        };

                        nockchain_math::mary_transpose::avx512::mary_transpose_offset_1_blocked_8x8_nt::<32>(
                            black_box(fpolys),
                            black_box(&mut res),
                        );
                    },
                );
            });
        }


        // AVX2 optimizations
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {

        }

        group.finish();
    }
}

criterion_group!(benches, bench_transpose);
criterion_main!(benches);
