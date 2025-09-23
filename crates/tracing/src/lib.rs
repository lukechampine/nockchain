extern crate tracing_base;

#[cfg(feature = "stealthy")]
extern crate noop_attr;

#[cfg(feature = "stealthy")]
pub use noop_attr::noop as instrument;
pub use tracing_base::*;

#[cfg(feature = "stealthy")]
pub mod noop {
    #[derive(Clone, Copy)]
    pub struct NoopSpan;

    impl NoopSpan {
        pub fn in_scope<T>(self, f: impl FnOnce() -> T) -> T {
            f()
        }
    }
}

#[cfg(feature = "stealthy")]
#[macro_export]
macro_rules! info_span {
    ($($tt:tt)*) => {
        $crate::noop::NoopSpan
    };
}

#[cfg(feature = "stealthy")]
pub mod log {
    #[macro_export]
    macro_rules! error {
        ($($tt:tt)*) => {
            ()
        };
    }
    #[macro_export]
    macro_rules! warn {
        ($($tt:tt)*) => {
            ()
        };
    }
    #[macro_export]
    macro_rules! debug {
        ($($tt:tt)*) => {
            ()
        };
    }
    #[macro_export]
    macro_rules! info {
        ($($tt:tt)*) => {
            ()
        };
    }
    #[macro_export]
    macro_rules! trace {
        ($($tt:tt)*) => {
            ()
        };
    }
    pub use super::{debug, error, info, trace, warn};
}
