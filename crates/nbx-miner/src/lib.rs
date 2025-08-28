#[cfg(feature = "client")]
pub mod client;
pub mod proto;
pub mod server;
pub mod shared;
pub mod proxy;
#[cfg(feature = "client")]
pub mod poker;
pub mod client_base;

#[cfg(feature = "stealthy")]
pub mod metrics {
    pub struct NoopMetrics;

    impl NoopMetrics {
        pub fn set(&self, _: f64) {}
        pub fn increment(&self, _: u64) {}
        pub fn record(&self, _: f64) {}
    }

    #[macro_export]
    macro_rules! gauge {
        ($($tt:tt)*) => { $crate::metrics::NoopMetrics }
    }

    #[macro_export]
    macro_rules! counter {
        ($($tt:tt)*) => { $crate::metrics::NoopMetrics }
    }

    #[macro_export]
    macro_rules! histogram {
        ($($tt:tt)*) => { $crate::metrics::NoopMetrics }
    }

    pub use super::{gauge, counter, histogram};
}

#[cfg(not(feature = "stealthy"))]
pub mod metrics {
    pub use ::metrics::*;
}
