use core::num::NonZeroU64;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::collections::{BTreeMap, btree_map::Entry};
use std::sync::atomic::{AtomicU32, Ordering};

use either::Either;
use nbx_shaders::get_shader_module;
use crate::log::*;
use wgpu::{Backends, Buffer, Queue, DeviceType, SubmissionIndex, CommandBuffer};
use zkvm_jetpack::form::Melt;
use crate::engine::Engine;
use crate::gpu::queue::GpuQueue;
use self::codewords::{BpShiftUniform, Hash10FixedPrependUniform, HashFixedMultipleUniform, HashVarlenMultipleUniform, MaryTransposeUniform, MontUniform};
use self::deep::{FpAccumUniform, FpHadamardSamepolyUniform, WeightedComboFinishUniform};
use self::substitute::{AccumUniform, MulUniform, SubstituteIterOps};
use self::util::PNttUniform;
use super::substitute::SubstituteEngine;
use crate::hash::{HashEngine, NounDigest, ReduceOp, VariableReduceOp};
use crate::instruments::local_instruments;

pub use handle::GpuHandle;
pub use registry::GpuRegistry;

mod hash;
mod substitute;
mod codewords;
mod deep;

pub(crate) mod util;
mod queue;
mod registry;
mod handle;

struct Pipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

static GPUS: Mutex<BTreeMap<(Option<String>, usize), (wgpu::Device, wgpu::Queue)>> = Mutex::new(BTreeMap::new());

pub const DEFAULT_GPU_QUEUE_SIZE: usize = 5;

pub struct Gpu {
    device: wgpu::Device,
    wgpu_queue: wgpu::Queue,
    work_queue: GpuQueue,
    hash_fixed: Pipeline,
    hash_variable: Pipeline,
    substitute_mul: Pipeline,
    substitute_accum: Pipeline,
    bp_shift: Pipeline,
    p_ntt_swap: Pipeline,
    bp_ntt: Pipeline,
    fp_ntt: Pipeline,
    mary_transpose: Pipeline,
    montify: Pipeline,
    montyred: Pipeline,
    hash_varlen_multiple: Pipeline,
    hash_fixed_multiple: Pipeline,
    hash_10_fixedprepend: Pipeline,
    weighted_combo_finish: Pipeline,
    fp_hadamard_samepoly: Pipeline,
    fp_accum: Pipeline,
    debug_capture: Arc<Mutex<Option<bool>>>,
}

impl Gpu {
    fn new(gpu_name_filter: Option<&str>, gpu_idx: usize, queue_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut cache = GPUS.lock()?;
        let (device, wgpu_queue) = match cache.entry((gpu_name_filter.map(String::from), gpu_idx)) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());

                let mut adapters = instance.enumerate_adapters(Backends::from_env().unwrap_or_default());

                fn dt_to_weight(dt: DeviceType) -> usize {
                    match dt {
                        DeviceType::Other => 0,
                        DeviceType::Cpu => 1,
                        DeviceType::VirtualGpu => 2,
                        DeviceType::IntegratedGpu => 3,
                        DeviceType::DiscreteGpu => 4,
                    }
                }

                adapters.sort_by_key(|v| !dt_to_weight(v.get_info().device_type));

                for adapter in &adapters {
                    trace!("Potential adapter {:?}", adapter.get_info());
                }

                let adapter = if let Some(target_gpu) = gpu_name_filter {
                    adapters.into_iter().filter(|v| v.get_info().name.contains(target_gpu)).nth(gpu_idx)
                } else {
                    adapters.into_iter().nth(gpu_idx)
                }.ok_or("Adapter not found")?;

                debug!("Adapter found {:?}", adapter.get_info());

                let downlevel_capabilities = adapter.get_downlevel_capabilities();
                if !downlevel_capabilities
                    .flags
                        .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
                {
                    return Err("No compute shader support".into());
                }

                let required_limits = wgpu::Limits::downlevel_defaults();

                let required_features = wgpu::Features::SHADER_INT64;

                #[cfg(all(feature = "shader_passthrough", target_os = "linux"))]
                let required_features = required_features | wgpu::Features::SPIRV_SHADER_PASSTHROUGH;

                #[cfg(all(feature = "shader_passthrough", target_os = "macos"))]
                let required_features = required_features | wgpu::Features::MSL_SHADER_PASSTHROUGH;

                let (device, queue) =
                    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                        label: None,
                        required_features,
                        required_limits,
                        memory_hints: wgpu::MemoryHints::MemoryUsage,
                        trace: wgpu::Trace::Off,
                    }))?;

                e.insert((device, queue)).clone()
            }
        };
        core::mem::drop(cache);

        debug!("Aquired device and queue");

        // Shader related

        let [hash_fixed, hash_variable, substitute_mul, substitute_accum, bp_shift, p_ntt_swap, bp_ntt, fp_ntt, mary_transpose, montify, montyred, hash_varlen_multiple, hash_fixed_multiple, hash_10_fixedprepend, weighted_combo_finish, fp_hadamard_samepoly, fp_accum] = [
            (
                Some(size_of::<ReduceOp>()),
                "hash_fixed",
                Either::Left(1),
                &[size_of::<WgOffsets>()] as &[_],
            ),
            (
                Some(size_of::<VariableReduceOp>()),
                "hash_variable",
                Either::Left(1),
                &[size_of::<WgOffsets>()],
            ),
            (
                Some(size_of::<SubstituteIterOps>()),
                "substitute_mul",
                Either::Right(&[true, false][..]),
                &[size_of::<MulUniform>()],
            ),
            (
                None,
                "substitute_accum",
                Either::Right(&[false]),
                &[size_of::<AccumUniform>()],
            ),
            (
                None,
                "bp_shift",
                Either::Right(&[false, false]),
                &[size_of::<BpShiftUniform>()],
            ),
            (
                None,
                "p_ntt_swap",
                Either::Right(&[false, false]),
                &[size_of::<PNttUniform>()],
            ),
            (
                None,
                "bp_ntt",
                Either::Right(&[false]),
                &[size_of::<PNttUniform>(), size_of::<u32>()],
            ),
            (
                None,
                "fp_ntt",
                Either::Right(&[false]),
                &[size_of::<PNttUniform>(), size_of::<u32>()],
            ),
            (
                None,
                "mary_transpose",
                Either::Right(&[false]),
                &[size_of::<MaryTransposeUniform>()],
            ),
            (
                None,
                "montify",
                Either::Right(&[false]),
                &[size_of::<MontUniform>()],
            ),
            (
                None,
                "montyred",
                Either::Right(&[false]),
                &[size_of::<MontUniform>()],
            ),
            (
                None,
                "hash_varlen_multiple",
                Either::Right(&[true]),
                &[size_of::<HashVarlenMultipleUniform>()],
            ),
            (
                None,
                "hash_fixed_multiple",
                Either::Right(&[true]),
                &[size_of::<HashFixedMultipleUniform>()],
            ),
            (
                None,
                "hash_10_fixedprepend",
                Either::Right(&[true]),
                &[size_of::<Hash10FixedPrependUniform>()],
            ),
            (
                None,
                "weighted_combo_finish",
                Either::Right(&[false, false, false]),
                &[size_of::<WeightedComboFinishUniform>()],
            ),
            (
                None,
                "fp_hadamard_samepoly",
                Either::Right(&[false]),
                &[size_of::<FpHadamardSamepolyUniform>()],
            ),
            (
                None,
                "fp_accum",
                Either::Right(&[false]),
                &[size_of::<FpAccumUniform>()],
            ),
        ]
        .map(|(ops_sz, source_label, inputs, uniform_sz)| {
            debug!("Shader module");
            let module = get_shader_module(&device, source_label);

            let mut entries = vec![];

            for sz in uniform_sz.iter().copied() {
                entries.push(
                    // Offsets
                    wgpu::BindGroupLayoutEntry {
                        binding: entries.len() as _,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            // This is the size of a single element in the buffer.
                            min_binding_size: Some(NonZeroU64::new(sz as _).unwrap()),
                            has_dynamic_offset: false,
                        },
                        count: None,
                    },
                );
            }

            if let Some(sz) = ops_sz {
                entries.push(
                    // Ops buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: entries.len() as _,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            // This is the size of a single element in the buffer.
                            min_binding_size: Some(NonZeroU64::new(sz as _).unwrap()),
                            has_dynamic_offset: false,
                        },
                        count: None,
                    },
                );
            }

            entries.push(
                // Output buffer
                wgpu::BindGroupLayoutEntry {
                    binding: entries.len() as _,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        min_binding_size: None,
                        has_dynamic_offset: false,
                    },
                    count: None,
                },
            );

            for read_only in inputs
                .map_left(|sz| (0..sz).map(|_| true))
                .map_right(|v| v.iter().copied())
            {
                entries.push(
                    // Input buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: entries.len() as _,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only },
                            min_binding_size: None,
                            has_dynamic_offset: false,
                        },
                        count: None,
                    },
                )
            }

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries,
                });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            debug!("Pipeline layout");

            // The pipeline is the ready-to-go program state for the GPU. It contains the shader modules,
            // the interfaces (bind group layouts) and the shader entry point.
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

            Pipeline {
                pipeline,
                bind_group_layout,
            }
        });

        debug!("Pipeline");

        Ok(Self {
            device,
            wgpu_queue,
            work_queue: GpuQueue::new(queue_size),
            hash_fixed,
            hash_variable,
            substitute_mul,
            substitute_accum,
            bp_shift,
            p_ntt_swap,
            bp_ntt,
            fp_ntt,
            mary_transpose,
            montify,
            montyred,
            hash_varlen_multiple,
            hash_fixed_multiple,
            hash_10_fixedprepend,
            weighted_combo_finish,
            fp_hadamard_samepoly,
            fp_accum,
            debug_capture: Mutex::new(Some(
                std::env::var("GPU_DEBUGGER").as_deref().unwrap_or("0") != "0",
            ))
            .into(),
        })
    }

    pub fn can_submit_work(&self) -> bool {
        self.work_queue.can_submit_work()
    }

    pub fn finished_work(&self) {
        self.work_queue.finished_work();
    }
}

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Debug)]
#[repr(C)]
struct WgOffsets {
    ops: u32,
    num_ops: u32,
    inp: u32,
    out: u32,
}

fn get_engine() -> HashEngine {
    let mut engine = HashEngine::default();

    let stages = std::env::var("TEST_STAGES")
        .as_deref()
        .unwrap_or("23")
        .parse::<usize>()
        .unwrap();

    engine.ensure_stages(stages);

    for i in 0..(1 << (stages - 1)) {
        engine.reserve_pair(stages);
        if i == 0 {
            engine.push_hashes::<Melt>(
                stages,
                &[[Melt(0xfffffffe00000001); 5], NounDigest::default()][..],
            );
        } else {
            engine.push_hashes::<Melt>(stages, &[NounDigest::default(); 2][..]);
        }
    }

    engine.push_pair(0, 0, 5);

    for l in 1..stages {
        for i in (0..(1 << l)).step_by(2) {
            engine.reserve_pair(l);
            let pos = i * 10;
            engine.push_pair(l, pos, pos + 5);
            engine.push_pair(l, pos + 10, pos + 15);
        }
    }

    // add some test data to trigger variable hashing
    let list: NounDigest = [Melt(1), Melt(2), Melt(3), Melt(4), Melt(5)];
    engine.push_list(0, [list; 1].into_iter()).unwrap();

    engine
}

pub fn gpu_test() -> Result<(), Box<dyn std::error::Error>> {
    use crate::parallel::prelude::*;

    println!("Reducing");

    if true {
        let t = Instant::now();
        let hash_engines = (0..std::env::var("GPU_SUBMISSIONS")
            .as_deref()
            .unwrap_or("1")
            .parse::<usize>()
            .unwrap())
            .into_par_iter()
            .map(|_| get_engine())
            .collect::<Vec<_>>();
        println!("Hash engines: {:.02}", t.elapsed().as_secs_f64());
        let t2 = Instant::now();
        let gpu_submissions = hash_engines
            .into_par_iter()
            .map(|engine| Submittable::submit(engine, get_available_gpu().unwrap()))
            .map(Submission::enqueue)
            .collect::<Vec<_>>();
        println!(
            "Submitted all: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();
        let gpu_buffers = gpu_submissions
            .into_iter()
            .map(Submission::finish)
            .collect::<Vec<_>>();
        let gpu_buffer = &gpu_buffers[0];
        println!("{:?}", gpu_buffer);
        println!(
            "GPU Time: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
    }

    if true {
        let t = Instant::now();
        let cpu_buffer = get_engine().reduce_cpu();
        println!("{:?}", cpu_buffer);
        println!("CPU Time: {:.02}", t.elapsed().as_secs_f64());
    }

    Ok(())
}

pub fn gpu_sub_test(engine: SubstituteEngine<Melt>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::parallel::prelude::*;

    println!("Substituting on poly size {}", engine.poly_len());

    let t = Instant::now();
    let engines = (0..std::env::var("GPU_SUBMISSIONS")
        .as_deref()
        .unwrap_or("1")
        .parse::<usize>()
        .unwrap())
        .into_par_iter()
        .map(|_| engine.clone())
        .collect::<Vec<_>>();
    println!("Substitution engines: {:.02}", t.elapsed().as_secs_f64());
    let t2 = Instant::now();
    let gpu_submissions = engines
        .into_par_iter()
        .map(|engine| Submittable::submit(engine, get_available_gpu().unwrap()))
        .map(Submission::enqueue)
        .collect::<Vec<_>>();
    println!(
        "Submitted all: {:.02}, {:.02}",
        t.elapsed().as_secs_f64(),
        t2.elapsed().as_secs_f64()
    );
    let t2 = Instant::now();
    let gpu_buffers = gpu_submissions
        .into_iter()
        .map(Submission::finish)
        .collect::<Vec<_>>();
    let gpu_buffer = &gpu_buffers[0];
    println!("{:?}", &gpu_buffer[0][..10]);
    println!(
        "GPU Time: {:.02}, {:.02}",
        t.elapsed().as_secs_f64(),
        t2.elapsed().as_secs_f64()
    );

    let t = Instant::now();
    let (cpu_buffer, _) = engine.clone().reduce_cpu();
    println!("{:?}", &cpu_buffer[0][..10]);
    println!("CPU Time: {:.02}", t.elapsed().as_secs_f64());

    println!("Match? {:?}", gpu_buffer == &cpu_buffer);

    Ok(())
}

struct DebugHandle {
    debug: Option<Arc<Mutex<Option<bool>>>>,
}

impl From<Option<Arc<Mutex<Option<bool>>>>> for DebugHandle {
    fn from(debug: Option<Arc<Mutex<Option<bool>>>>) -> Self {
        Self {
            debug
        }
    }
}

impl Drop for DebugHandle {
    fn drop(&mut self) {
        if let Some(debug) = self.debug.take() {
            *debug.lock().unwrap() = Some(true);
        }
    }
}

pub struct Submission<T: FromBuffer, O> {
    gpu: GpuHandle,
    obj: O,
    downloads: Vec<Buffer>,
    debug: DebugHandle,
    _download_convert: PhantomData<T>,
    mdata: T::Metadata,
}

impl<T: FromBuffer> Submission<T, CommandBuffer> {
    #[tracing::instrument(skip_all)]
    pub fn enqueue(self) -> Submission<T, SubmissionIndex> {
        let Self {
            gpu,
            obj: command_buffer,
            downloads,
            debug,
            _download_convert,
            mdata,
        } = self;

        let inst = local_instruments();
        let _probe = inst.gpu_enqueue_probe();

        let si = gpu.wgpu_queue.submit([command_buffer]);

        Submission {
            gpu,
            obj: si,
            downloads,
            debug,
            _download_convert,
            mdata
        }
    }
}

impl<T: FromBuffer> Submission<T, SubmissionIndex> {
    #[tracing::instrument(skip_all)]
    pub fn finish(self) -> T {
        let Self {
            gpu,
            obj: _,
            downloads,
            debug,
            _download_convert: _,
            mdata,
        } = self;

        let inst = local_instruments();
        let probe = inst.gpu_finish_probe();

        let tracker = Arc::new(CompletionTracker::new(downloads.len()));

        let buffer_slices = downloads
            .iter()
            .enumerate()
            .map(|(i, download)| {
                let buffer_slice = download.slice(..);
                let tracker_clone = tracker.clone();

                buffer_slice.map_async(wgpu::MapMode::Read, move |_| {
                    tracker_clone.mark_ready(i);
                });

                buffer_slice
            })
            .collect::<Vec<_>>();

        debug!("Map request");

        loop {
            gpu.device.poll(wgpu::PollType::Poll).unwrap();

            if tracker.is_all_ready() { break; }

            rayon::yield_now();
        }

        debug!("Polled");

        let buffer_slices = buffer_slices
            .into_iter()
            .map(|b| b.get_mapped_range())
            .collect::<Vec<_>>();

        if debug.debug.is_some() {
            unsafe { gpu.device.stop_graphics_debugger_capture() };
        }

        core::mem::drop(probe);

        T::from_buffers(&buffer_slices[..], &mdata)
    }
}

pub trait FromBuffer {
    type Metadata;

    fn from_buffers<T: AsRef<[u8]>>(b: &[T], mdata: &Self::Metadata) -> Self;
}

pub trait Submittable: Sized {
    type Output: FromBuffer;

    fn submit(self, gpu: GpuHandle) -> Submission<Self::Output, CommandBuffer>;

    fn gpu_process(self, gpu: GpuHandle) -> Self::Output {
        self.submit(gpu).enqueue().finish()
    }
}

pub fn get_available_gpu() -> Option<GpuHandle> {
    GpuRegistry::get().get_available_gpu()
}

struct CompletionTracker {
    ready_mask: AtomicU32,
    target_mask: u32,
}

impl CompletionTracker {
    fn new(num_buffers: usize) -> Self {
        assert!(num_buffers < 32);
        Self {
            ready_mask: AtomicU32::new(0),
            target_mask: (1 << num_buffers) - 1,
        }
    }

    fn mark_ready(&self, index: usize) {
        self.ready_mask.fetch_or(1 << index, Ordering::Release);
    }

    fn is_all_ready(&self) -> bool {
        self.ready_mask.load(Ordering::Relaxed) == self.target_mask
    }
}
