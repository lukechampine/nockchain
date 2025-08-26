use std::sync::atomic::{AtomicUsize, Ordering};

pub struct GpuQueue {
    available_slots: AtomicUsize,
}

impl GpuQueue {
    pub fn new(queue_size: usize) -> Self {
        Self {
            available_slots: AtomicUsize::new(queue_size),
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn finished_work(&self) {
        self.available_slots.fetch_add(1, Ordering::AcqRel);
    }

    #[tracing::instrument(skip_all)]
    pub fn can_submit_work(&self) -> bool {
        let current = self.available_slots.load(Ordering::Acquire);
        if current == 0 {
            return false;
        }

        // Decrementing the counter can fail due to either (1) concurrent access or (2) because the
        //  queue is already full. Either way, the current thread should use the CPU to process the
        //  piece of work.
        self.available_slots.compare_exchange_weak(
            current,
            current - 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ).is_ok()
    }
}
