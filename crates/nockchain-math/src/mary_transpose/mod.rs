#[cfg(target_arch = "aarch64")]
pub mod neon;
pub mod scalar;
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
pub mod avx2;
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
pub mod avx512;
