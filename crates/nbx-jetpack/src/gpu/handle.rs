use std::sync::Arc;
use std::ops::Deref;
use crate::gpu::Gpu;

pub struct GpuHandle(Arc<Gpu>);

impl GpuHandle {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        Self(gpu)
    }
}

impl Deref for GpuHandle {
    type Target = Arc<Gpu>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for GpuHandle {
    fn drop(&mut self) {
        // Automatically mark the work as completed the moment the guard is dropped.
        // This ensures that there are no accidentally dangling places reserved in the GPU queue.
        self.0.finished_work();
    }
}
