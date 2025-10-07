#[cfg(feature = "client")]
pub mod client;
pub mod client_base;
#[cfg(feature = "db")]
pub mod db;
pub mod device;
#[cfg(feature = "client")]
pub mod poker;
pub mod proto;
pub mod proxy;
pub mod server;
pub mod shared;

macro_rules! log {
    ($mode:ident, $($tt:tt)*) => {
        tracing::unfiltered_tracing::log::$mode! {
            target: LOG_TARGET,
            $($tt)*
        }
    };
}
use clap::ColorChoice;
pub(crate) use log;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

const DEFAULT_LOG_FILTER: &str = "info";

#[cfg(feature = "production")]
struct PrefixFilter;

#[cfg(feature = "production")]
impl<S: Subscriber> Layer<S> for PrefixFilter {
    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        metadata.target().starts_with(obfstr::obfstr!("nbx::"))
    }
}

pub fn init_default_tracing(colors: ColorChoice) {
    let use_ansi = colors == ColorChoice::Auto || colors == ColorChoice::Always;

    let filter = EnvFilter::new(
        std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_string()),
    );

    let sub = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_ansi(use_ansi)
                .with_target(true)
                .with_level(true),
        );

    #[cfg(feature = "production")]
    let sub = sub
        .with(PrefixFilter);

    sub
        .with(filter)
        .init();
}

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
        ($($tt:tt)*) => {
            $crate::metrics::NoopMetrics
        };
    }

    #[macro_export]
    macro_rules! counter {
        ($($tt:tt)*) => {
            $crate::metrics::NoopMetrics
        };
    }

    #[macro_export]
    macro_rules! histogram {
        ($($tt:tt)*) => {
            $crate::metrics::NoopMetrics
        };
    }

    pub use super::{counter, gauge, histogram};
}

#[cfg(not(feature = "stealthy"))]
pub mod metrics {
    pub use ::metrics::*;
}
