use std::sync::{Arc, OnceLock};

use tracing::warn;

use crate::gpu::{Gpu, GpuHandle};

pub struct GpuRegistry {
    gpus: Vec<Arc<Gpu>>,
}

static GPU_REGISTRY: OnceLock<GpuRegistry> = OnceLock::new();

impl GpuRegistry {
    pub fn builder() -> GpuRegistryBuilder {
        GpuRegistryBuilder::new()
    }

    pub fn get() -> &'static GpuRegistry {
        GPU_REGISTRY.get_or_init(|| {
            warn!("GpuRegistry not initialized, falling back to CPU computations only");
            GpuRegistry { gpus: vec![] }
        })
    }

    pub fn get_available_gpu(&self) -> Option<GpuHandle> {
        // Find the first GPU that can has space in the queue for an additional item.
        self.gpus
            .iter()
            .find(|gpu| gpu.can_submit_work())
            .map(|gpu| GpuHandle::new(Arc::clone(gpu)))
    }
}

pub struct GpuRegistryBuilder {
    gpus: Vec<Arc<Gpu>>,
}

impl GpuRegistryBuilder {
    fn new() -> Self {
        Self { gpus: Vec::new() }
    }

    pub fn add_gpu(
        mut self,
        gpu_name_filter: Option<&str>,
        gpu_idx: usize,
        queue_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let gpu = Arc::new(Gpu::new(gpu_name_filter, gpu_idx, queue_size)?);
        self.gpus.push(gpu);
        Ok(self)
    }

    pub fn build(self) -> Result<(), Box<dyn std::error::Error>> {
        let registry = GpuRegistry { gpus: self.gpus };
        GPU_REGISTRY
            .set(registry)
            .map_err(|_| "GPU registry already initialized")?;
        Ok(())
    }
}
