use std::cell::RefCell;

#[cfg(feature = "validate-gpu")]
use {crate::log::*, std::time::Instant};

#[cfg(feature = "gpu")]
use crate::gpu;

thread_local! {
    static IS_IN_EXECUTION: RefCell<bool> = const { RefCell::new(false) };
}

pub trait Engine: Sized + Clone {
    type Output: PartialEq;

    #[tracing::instrument(skip_all)]
    fn reduce(self) -> Self::Output {
        // If the GPU is not enabled, return the CPU result directly
        #[cfg(not(feature = "gpu"))]
        return self.reduce_cpu();

        #[cfg(feature = "gpu")]
        {
            let is_subproblem = IS_IN_EXECUTION.with(|in_exec| {
                let was_executing = *in_exec.borrow();
                *in_exec.borrow_mut() = true;
                was_executing
            });

            // When this is a subproblem of another engine, always use the CPU to prevent using the
            //  GPU for small tasks.
            if is_subproblem {
                return self.reduce_cpu();
            }

            let result = self.reduce_gpu_impl();

            IS_IN_EXECUTION.with(|in_exec| {
                *in_exec.borrow_mut() = false;
            });

            result
        }
    }

    #[cfg(feature = "gpu")]
    fn reduce_gpu_impl(self) -> Self::Output {
        #[cfg(feature = "validate-gpu")]
        return self.validate_gpu();

        if let Some(gpu) = gpu::get_available_gpu() {
            return self.reduce_gpu(gpu);
        }

        // If there is no available GPU, fallback to the CPU
        self.reduce_cpu()
    }

    #[cfg(feature = "validate-gpu")]
    fn validate_gpu(self) -> Self::Output {
        let gpu = std::iter::repeat_with(|| gpu::get_available_gpu())
            .find_map(|gpu| gpu)
            .unwrap();

        // Reduce the problem with both the CPU and GPU to compare the results. Any subproblems of
        //  the engine will always be reduced with the CPU.
        let cpu_start = Instant::now();
        let cpu_result = self.clone().reduce_cpu();
        let cpu_duration = cpu_start.elapsed();

        let gpu_start = Instant::now();
        let gpu_result = self.reduce_gpu(gpu);
        let gpu_duration = gpu_start.elapsed();

        // Emit warnings if the CPU and GPU results are different
        if cpu_result != gpu_result {
            warn!(
                "The result of the CPU and GPU are different for the `{}` operation",
                std::any::type_name::<Self>(),
            )
        }

        info!(
            "{} CPU to GPU speedup: {:.03}s -> {:.03}s => {:.02} seconds speedup / GPU second",
            std::any::type_name::<Self>(),
            cpu_duration.as_secs_f64(),
            gpu_duration.as_secs_f64(),
            (cpu_duration.as_secs_f64() - gpu_duration.as_secs_f64()) / gpu_duration.as_secs_f64(),
        );

        // Even if the result is computed using a GPU for validation, always return the CPU
        //  result as it is considered more reliable.
        cpu_result
    }

    fn reduce_cpu(self) -> Self::Output;

    #[cfg(feature = "gpu")]
    fn reduce_gpu(self, gpu: gpu::GpuHandle) -> Self::Output;
}
