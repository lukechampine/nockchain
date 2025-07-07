use core::num::NonZeroU64;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use nbx_shaders::get_shader_module;
use tracing::*;

use self::hash::HashSubmission;
use crate::form::Melt;
use crate::jets::nbx::hash::{HashEngine, NounDigest, ReduceOp, VariableReduceOp};

use super::substitute::SubstituteEngine;

mod hash;

struct Pipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    hash_fixed: Pipeline,
    hash_variable: Pipeline,
    debug_capture: Arc<Mutex<Option<bool>>>,
}

impl Gpu {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;

        debug!("Adapter found {:?}", adapter.get_info());

        let downlevel_capabilities = adapter.get_downlevel_capabilities();
        if !downlevel_capabilities
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            return Err("No compute shader support".into());
        }

        let required_limits = wgpu::Limits::downlevel_defaults();

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::SHADER_INT64
                    | wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
                required_limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            }))?;

        debug!("Aquired device and queue");

        // Shader related

        let [hash_fixed, hash_variable] = [
            (core::mem::size_of::<ReduceOp>(), "hash_fixed"),
            (
                core::mem::size_of::<VariableReduceOp>(),
                // TODO: hash_variable
                "hash_fixed",
            ),
        ]
        .map(|(sz, source_label)| {
            debug!("Shader module");
            let module = get_shader_module(&device, source_label);

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        // Input buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                // This is the size of a single element in the buffer.
                                min_binding_size: Some(NonZeroU64::new(8).unwrap()),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                        // Ops buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                // This is the size of a single element in the buffer.
                                min_binding_size: Some(NonZeroU64::new(sz as _).unwrap()),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                        // Offsets
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                // This is the size of a single element in the buffer.
                                min_binding_size: Some(
                                    NonZeroU64::new(core::mem::size_of::<WgOffsets>() as _)
                                        .unwrap(),
                                ),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                        // Output buffer
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                // This is the size of a single element in the buffer.
                                min_binding_size: Some(NonZeroU64::new(8).unwrap()),
                                has_dynamic_offset: false,
                            },
                            count: None,
                        },
                    ],
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
            queue,
            hash_fixed,
            hash_variable,
            debug_capture: Mutex::new(Some(
                std::env::var("GPU_DEBUGGER").as_deref().unwrap_or("0") != "0",
            ))
            .into(),
        })
    }
}

static GPU: OnceLock<Gpu> = OnceLock::new();

fn get_gpu() -> &'static Gpu {
    GPU.get_or_init(|| Gpu::new().unwrap())
}

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
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

    engine
}

fn cpu_reduce(engine: HashEngine) -> Vec<NounDigest> {
    /*let mut cur: Vec<Melt> = vec![];
    let mut stages = engine.destruct();

    while let Some(ReduceStage {
        ops_variable: _,
        ops_fixed,
        mut out,
    }) = stages.pop()
    {
        use rayon::prelude::*;
        struct MeltSlice(*mut Melt);
        unsafe impl Send for MeltSlice {}
        unsafe impl Sync for MeltSlice {}
        let out_ptr = MeltSlice(out.as_mut_ptr());
        ops_fixed.into_par_iter().for_each(|ReduceOp { source, destination }| {
            let out = &out_ptr;
            (0..5).into_iter().for_each(|off| {
                let val = cur[(source + off) as usize].0;
                let mut tmp = [Melt(0); tip5::STATE_SIZE];
                tmp[0] = Melt(val);
                for i in 0..1 {
                    tip5::permute(&mut tmp);
                }
                unsafe { *out.0.add((destination + off) as usize) = tmp[0] };
            });
        });
        cur = out;
    }

    assert_eq!(cur.len() % DIGEST_LENGTH, 0);
    let p = cur.as_mut_ptr();
    let l = cur.len() / DIGEST_LENGTH;
    let c = cur.capacity() / DIGEST_LENGTH;
    core::mem::forget(cur);
    unsafe { Vec::from_raw_parts(p as *mut NounDigest, l, c) }*/
    engine.reduce()
}

pub fn gpu_test() -> Result<(), Box<dyn std::error::Error>> {
    use rayon::prelude::*;

    let _ = get_gpu();

    println!("Reducing");

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
        .map(hash::reduce)
        .collect::<Vec<_>>();
    println!("Submitted all: {:.02}, {:.02}", t.elapsed().as_secs_f64(), t2.elapsed().as_secs_f64());
    let t2 = Instant::now();
    let gpu_buffers = gpu_submissions
        .into_iter()
        .map(HashSubmission::finish)
        .collect::<Vec<_>>();
    let gpu_buffer = &gpu_buffers[0];
    println!("{:?}", gpu_buffer);
    println!("GPU Time: {:.02}, {:.02}", t.elapsed().as_secs_f64(), t2.elapsed().as_secs_f64());

    let t = Instant::now();
    let cpu_buffer = cpu_reduce(get_engine());
    println!("{:?}", cpu_buffer);
    println!("CPU Time: {:.02}", t.elapsed().as_secs_f64());

    Ok(())
}

pub fn gpu_sub_test(engine: SubstituteEngine<Melt>) -> Result<(), Box<dyn std::error::Error>> {

    let _ = get_gpu();

    println!("Substituting on poly size {}", engine.poly_len());

    /*let t = Instant::now();
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
        .map(hash::reduce)
        .collect::<Vec<_>>();
    println!("Submitted all: {:.02}, {:.02}", t.elapsed().as_secs_f64(), t2.elapsed().as_secs_f64());
    let t2 = Instant::now();
    let gpu_buffers = gpu_submissions
        .into_iter()
        .map(HashSubmission::finish)
        .collect::<Vec<_>>();
    let gpu_buffer = &gpu_buffers[0];
    println!("{:?}", gpu_buffer);
    println!("GPU Time: {:.02}, {:.02}", t.elapsed().as_secs_f64(), t2.elapsed().as_secs_f64());*/

    let t = Instant::now();
    let (cpu_buffer, _) = engine.clone().reduce();
    println!("{:?}", &cpu_buffer[0][..10]);
    println!("CPU Time: {:.02}", t.elapsed().as_secs_f64());

    Ok(())
}
