use crate::form::tip5::DIGEST_LENGTH;
use crate::form::{montiply, Melt};
use crate::jets::nbx::three::{ReduceOp, ReduceStage, VariableReduceOp};

use super::three::{HashEngine, NounDigest};
use core::num::NonZeroU64;
use nbx_shaders::get_shader_module;
use nbx_tip5::tip5;
use std::sync::{mpsc, Arc, OnceLock};
use std::time::Instant;
use tracing::*;
use wgpu::util::DeviceExt;
use wgpu::{
    Label, PushConstantRange, ShaderModuleDescriptor, ShaderModuleDescriptorPassthrough,
    ShaderModuleDescriptorSpirV, ShaderSource, ShaderStages,
};

struct Pipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    hash_fixed: Pipeline,
    hash_variable: Pipeline,
    debug_capture: bool,
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

        let mut required_limits = wgpu::Limits::downlevel_defaults();
        // 1GB
        required_limits.max_buffer_size = 0x40000000;
        required_limits.max_storage_buffer_binding_size = 0x40000000;
        //required_limits.max_storage_buffer_binding_size = 0x8000000;// 0x40000000;

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
                        // Ops offset
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                // This is the size of a single element in the buffer.
                                min_binding_size: Some(NonZeroU64::new(4).unwrap()),
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
            debug_capture: std::env::var("GPU_DEBUGGER").as_deref().unwrap_or("0") != "0",
        })
    }
}

static GPU: OnceLock<Gpu> = OnceLock::new();

fn get_gpu() -> &'static Gpu {
    GPU.get_or_init(|| Gpu::new().unwrap())
}

pub fn reduce(engine: HashEngine) -> Vec<NounDigest> {
    let t = Instant::now();

    let mut inputs = vec![];

    let gpu = get_gpu();

    let mut stages = engine.destruct();
    let mut i = 0;

    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = max_workgroup_insts * workgroup_size;
    //let op_size = ops.size() as usize / ops_len;
    //let chunk_size = 65536 * workgroup_size * op_size;
    let mut offset_uniforms = vec![];

    while let Some(stage) = stages.pop() {
        let ReduceStage {
            ops_variable,
            ops_fixed,
            out,
        } = stage;

        let fixed_len = ops_fixed.len();
        let variable_len = ops_variable.len();

        debug!(
            "Stage: {} {} {}",
            out.len(),
            ops_fixed.len(),
            ops_variable.len()
        );

        if i == 0 {
            assert_eq!(fixed_len, 0);
            assert_eq!(variable_len, 0);
        }

        let max_offset = core::cmp::max(ops_fixed.len(), ops_variable.len()).saturating_sub(1);
        let chunk_idx = max_offset / chunk_size;

        while offset_uniforms.len() <= chunk_idx {
            let uniform = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("chunk {chunk_idx}")),
                    contents: bytemuck::cast_slice(&[(chunk_idx * chunk_size) as u32]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            offset_uniforms.push(uniform);
        }

        /*// We cannot have empty buffers
        if ops_variable.is_empty() {
            ops_variable.push(VariableReduceOp {
                inner: ReduceOp { source: 0, destination: 0 },
                len: 0,
            });
        }

        if ops_fixed.is_empty() {
            ops_fixed.push(VariableReduceOp {
                inner: ReduceOp { source: 0, destination: 0 },
                len: 0,
            });
        }*/

        let mut usage = wgpu::BufferUsages::STORAGE;

        if stages.is_empty() {
            usage |= wgpu::BufferUsages::COPY_SRC;
        }

        let output = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("out {i}")),
                contents: bytemuck::cast_slice(&out),
                usage,
            });

        let fixed = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("fixops {i}")),
                contents: bytemuck::cast_slice(&ops_fixed),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let variable = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("varops {i}")),
                contents: bytemuck::cast_slice(&ops_variable),
                usage: wgpu::BufferUsages::STORAGE,
            });

        inputs.push((output, ((fixed, fixed_len), (variable, variable_len))));
        i += 1;
    }

    debug!("Inputs");

    let download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: core::mem::size_of::<NounDigest>() as _,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    debug!("All buffers");

    if gpu.debug_capture {
        unsafe { gpu.device.start_graphics_debugger_capture() };
    }

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    for io in inputs.windows(2) {
        let [(input, _), (output, output_ops)] = io else {
            panic!()
        };
        let (fixed, variable) = output_ops;

        for ((ops, ops_len), pipeline) in [(fixed, &gpu.hash_fixed), (variable, &gpu.hash_variable)]
        {
            // Empty buffer, no-op
            if ops.size() == 0 {
                continue;
            }

            for i in (0..*ops_len).step_by(chunk_size) {
                let chunk_idx = i / chunk_size;

                let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: input.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: ops.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: offset_uniforms[chunk_idx].as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: output.as_entire_binding(),
                        },
                    ],
                });

                // Single compute pass
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });

                // Set the pipeline that we want to use
                compute_pass.set_pipeline(&pipeline.pipeline);
                // Set the bind group that we want to use
                compute_pass.set_bind_group(0, &bind_group, &[]);

                let chunk_len = core::cmp::min(*ops_len, (chunk_idx + 1) * chunk_size) - i;
                let workgroup_count = chunk_len.div_ceil(workgroup_size as _);
                compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
            }
        }
    }

    debug!("Compute passes");

    let output = inputs.pop().unwrap().0;

    encoder.copy_buffer_to_buffer(&output, 0, &download, 0, output.size());

    let command_buffer = encoder.finish();

    debug!("Command buffer");

    let si = gpu.queue.submit([command_buffer]);

    debug!("Submitted (in {:.02}s)", t.elapsed().as_secs_f64());

    let buffer_slice = download.slice(..);
    let (tx, rx) = mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |_| {
        debug!("Mapped");
        let _ = tx.send(());
    });

    debug!("Map request");

    gpu.device
        .poll(wgpu::PollType::WaitForSubmissionIndex(si))
        .unwrap();
    debug!("Polled");

    let _ = rx.recv().unwrap();

    if gpu.debug_capture {
        unsafe { gpu.device.stop_graphics_debugger_capture() };
    }

    let data = buffer_slice.get_mapped_range();
    let result: &[NounDigest] = bytemuck::cast_slice(&data);

    println!("Result: {result:?}");

    result.to_vec()
}

fn get_engine() -> HashEngine {
    let mut engine = HashEngine::default();

    let stages = 23;

    engine.ensure_stages(stages);

    for i in 0..(1 << stages) {
        if i == 0 {
            engine.push_hashes::<Melt>(
                stages,
                &[[Melt(0xfffffffe00000001); 5], NounDigest::default()][..],
            );
        } else {
            engine.push_hashes::<Melt>(stages, &[NounDigest::default(); 2][..]);
        }
    }

    for l in 0..stages {
        for i in 0..(1 << l) {
            let pos = i * 10;
            engine.push_pair(l, pos, pos + 5);
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
    let _ = get_gpu();

    println!("Reducing");

    let t = Instant::now();
    let gpu_buffer = reduce(get_engine());
    println!("{:?}", gpu_buffer);
    println!("GPU Time: {:.02}", t.elapsed().as_secs_f64());

    let t = Instant::now();
    let cpu_buffer = cpu_reduce(get_engine());
    println!("{:?}", cpu_buffer);
    println!("CPU Time: {:.02}", t.elapsed().as_secs_f64());

    Ok(())
}
