use std::cell::LazyCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

thread_local! {
    static INSTRUMENTS: LazyCell<Arc<Instruments>> = LazyCell::new(Default::default);
}

pub fn local_instruments() -> Arc<Instruments> {
    INSTRUMENTS.with(|v| (**v).clone())
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReadInstruments {
    pub gpu_enqueue_ms: u64,
    pub gpu_submit_ms: u64,
    pub gpu_finish_ms: u64,
}

impl ReadInstruments {
    pub fn since(self, prev: Self) -> Self {
        Self {
            gpu_enqueue_ms: self.gpu_enqueue_ms - prev.gpu_enqueue_ms,
            gpu_submit_ms: self.gpu_submit_ms - prev.gpu_submit_ms,
            gpu_finish_ms: self.gpu_finish_ms - prev.gpu_finish_ms,
        }
    }
}

#[derive(Debug, Default)]
pub struct Instruments {
    gpu_enqueue_ms: AtomicU64,
    gpu_submit_ms: AtomicU64,
    gpu_finish_ms: AtomicU64,
}

impl Instruments {
    #[must_use]
    pub(crate) fn gpu_enqueue_probe(&self) -> InstrumentProbe {
        InstrumentProbe::new(&self.gpu_enqueue_ms)
    }

    #[must_use]
    pub(crate) fn gpu_submit_probe(&self) -> InstrumentProbe {
        InstrumentProbe::new(&self.gpu_submit_ms)
    }

    #[must_use]
    pub(crate) fn gpu_finish_probe(&self) -> InstrumentProbe {
        InstrumentProbe::new(&self.gpu_finish_ms)
    }

    pub fn read(&self) -> ReadInstruments {
        ReadInstruments {
            gpu_enqueue_ms: self.gpu_enqueue_ms.load(Ordering::Relaxed),
            gpu_submit_ms: self.gpu_submit_ms.load(Ordering::Relaxed),
            gpu_finish_ms: self.gpu_finish_ms.load(Ordering::Relaxed),
        }
    }
}

impl<'a> Drop for InstrumentProbe<'a> {
    fn drop(&mut self) {
        let ms = self.instant.elapsed().as_millis() as u64;
        self.target.fetch_add(ms, Ordering::Relaxed);
    }
}

pub(crate) struct InstrumentProbe<'a> {
    target: &'a AtomicU64,
    instant: Instant,
}

impl<'a> InstrumentProbe<'a> {
    fn new(target: &'a AtomicU64) -> Self {
        Self {
            target,
            instant: Instant::now(),
        }
    }
}
