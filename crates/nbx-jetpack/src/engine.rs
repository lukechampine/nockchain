#[cfg(feature = "gpu")]
use crate::gpu;
#[cfg(feature = "validate-gpu")]
use crate::log::*;

pub trait Engine: Sized + Clone {
    type Output: PartialEq;

    #[tracing::instrument(skip_all)]
    fn reduce(self) -> Self::Output {
        // If the GPU is not enabled, return the CPU result directly
        #[cfg(not(feature = "gpu"))]
        return self.reduce_cpu();

        #[cfg(feature = "gpu")]
        {
            #[cfg(not(feature = "validate-gpu"))]
            {
                if let Some(gpu) = gpu::get_available_gpu() {
                    return self.reduce_gpu(gpu);
                }

                // If there is no available GPU, fallback to the CPU
                self.reduce_cpu()
            }

            #[cfg(feature = "validate-gpu")]
            {
                let mut gpu = None;

                while gpu.is_none() {
                    gpu = gpu::get_available_gpu();
                }

                let cpu_result = self.clone().reduce_cpu();
                let gpu_result = self.reduce_gpu(gpu.unwrap());

                // Emit warnings if the CPU and GPU results are different
                if cpu_result != gpu_result {
                    warn!(
                        "The result of the CPU and GPU are different for the `{}` operation",
                        std::any::type_name::<Self>(),
                    )
                }

                // Even if the result is computed using a GPU for validation, always return the CPU
                //  result as it is considered more reliable.
                cpu_result
            }
        }
    }

    fn reduce_cpu(self) -> Self::Output;

    #[cfg(feature = "gpu")]
    fn reduce_gpu(self, gpu: gpu::GpuHandle) -> Self::Output;
}

