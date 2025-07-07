use std::time::Instant;

use tracing::*;
use wgpu::util::DeviceExt;
use wgpu::Buffer;

use super::{get_gpu, FromBuffer, Submittable, WgOffsets};
use crate::jets::nbx::gpu::Submission;
use crate::jets::nbx::hash::{HashEngine, NounDigest, ReduceChunk};

impl FromBuffer for Vec<NounDigest> {
    fn from_buffer(b: &[u8]) -> Self {
        let result: &[NounDigest] = bytemuck::cast_slice(&b);
        println!("Result: {result:?}");
        result.to_vec()
    }
}

enum RunArgs {
    Fixed(FixedRunArgs),
    Variable(VariableRunArgs),
}

struct FixedRunArgs {
    input: Buffer,
    output: Buffer,
    fixed: Buffer,
    ops_len: usize,
    uniform: Buffer,
}

struct VariableRunArgs {}

impl Submittable for HashEngine {
    type Output = Vec<NounDigest>;

    fn submit(self) -> Submission<Self::Output> {
        let t = Instant::now();

        let gpu = get_gpu();

        let mut stages = self.destruct();

        let workgroup_size = 256;
        let max_workgroup_insts = 65535;
        let chunk_size = max_workgroup_insts * workgroup_size;
        /*let maxbuf = core::cmp::min(
            gpu.device.limits().max_storage_buffer_binding_size,
            gpu.device.limits().max_buffer_size,
        );*/
        //let op_size = ops.size() as usize / ops_len;
        //let chunk_size = 65536 * workgroup_size * op_size;

        // First, allocate buffers for all outputs
        let mut stage_buffers = vec![];
        for (i, s) in stages.iter().enumerate() {
            let mut usage = wgpu::BufferUsages::STORAGE;

            if i == 0 {
                usage |= wgpu::BufferUsages::COPY_SRC;
            }

            let mut out_bufs = vec![];
            for (
                o,
                ReduceChunk {
                    out,
                    ops_fixed,
                    ops_variable,
                    ..
                },
            ) in s.chunks.iter().enumerate()
            {
                let output = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("out {i}-{o}")),
                        contents: bytemuck::cast_slice(&out),
                        usage,
                    });

                let fixed = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("fixops {i}-{o}")),
                        contents: bytemuck::cast_slice(&ops_fixed),
                        usage: wgpu::BufferUsages::STORAGE,
                    });

                let variable = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("varops {i}-{o}")),
                        contents: bytemuck::cast_slice(&ops_variable),
                        usage: wgpu::BufferUsages::STORAGE,
                    });

                out_bufs.push((output, fixed, variable));
            }
            stage_buffers.push(out_bufs);
        }

        debug!("stage buffers: {:.02}", t.elapsed().as_secs_f64());
        let t2 = Instant::now();

        let mut prev_sb: Option<Vec<(Buffer, Buffer, Buffer)>> = None;
        let mut cur_inputs = vec![];
        let mut processed_stages = vec![];

        // Now, produce shader runs that fit the constraints of the buffer sizes and max workgroup sizes
        while let Some(mut stage) = stages.pop() {
            let sb = stage_buffers.pop().unwrap();

            let mut fixed_runs = vec![];

            let chunks = stage
                .chunks
                .iter()
                .map(|v| (v.out_start, v.ops_fixed.as_ptr(), v.ops_variable.as_ptr()))
                .collect::<Vec<_>>();
            let pairs = stage.split_chunks(cur_inputs[..].iter().map(|v: &Vec<_>| v.as_ref()));

            for (inp_idx, inp_at, input, chunk_idx, chunk) in pairs {
                // Compute the number of workgroup dispatches we'll have, and generate uniforms for all
                // of them.
                let inp_off = if inp_idx != usize::MAX {
                    unsafe { input.as_ptr().offset_from(cur_inputs[inp_idx].as_ptr()) }
                } else {
                    continue;
                };
                let fixed_off =
                    unsafe { chunk.ops_fixed.as_ptr().offset_from(chunks[chunk_idx].1) };
                let orig_out_start = chunks[chunk_idx].0;
                /*let var_off = unsafe {
                    chunk
                        .ops_variable
                        .as_ptr()
                        .offset_from(chunks[inp_idx].2)
                };*/

                if let Some(psb) = &prev_sb {
                    let (output, fixed, _) = &sb[chunk_idx];

                    for (chunk_idx, c) in chunk.ops_fixed.chunks(chunk_size).enumerate() {
                        let offs = WgOffsets {
                            ops: ((chunk_idx * chunk_size) + fixed_off as usize) as _,
                            num_ops: c.len() as _,
                            // We are binding the whole input buffer, hence the offset must be of the
                            // buffer.
                            inp: (inp_at - inp_off as usize) as _,
                            // Same with output - we need the original out_start
                            out: orig_out_start as _,
                        };

                        let uniform =
                            gpu.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: None, //Some(&format!("chunk {chunk_idx}")),
                                    contents: bytemuck::cast_slice(&[offs]),
                                    usage: wgpu::BufferUsages::UNIFORM,
                                });

                        fixed_runs.push(RunArgs::Fixed(FixedRunArgs {
                            input: psb[inp_idx].0.clone(),
                            output: output.clone(),
                            fixed: fixed.clone(),
                            ops_len: c.len(),
                            uniform,
                        }));
                    }
                } else {
                    unreachable!();
                }
            }

            processed_stages.push(fixed_runs);
            prev_sb = Some(sb);
            cur_inputs = stage.chunks.into_iter().map(|v| v.out).collect::<Vec<_>>();
        }

        debug!(
            "runs: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let download_size = prev_sb.as_ref().unwrap()[0].0.size();
        let download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: download_size, // core::mem::size_of::<NounDigest>() as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        debug!(
            "download: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let debug_capture_guard = gpu.debug_capture.try_lock();
        let debug_capture = *debug_capture_guard.as_deref().unwrap_or(&None);

        let debug = if debug_capture == Some(true) {
            unsafe { gpu.device.start_graphics_debugger_capture() };
            debug_capture_guard.unwrap().take();
            Some(gpu.debug_capture.clone())
        } else {
            core::mem::drop(debug_capture_guard);
            None
        };

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            // Single compute pass
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });

            for stage in processed_stages {
                for run in stage {
                    match run {
                        RunArgs::Fixed(runargs) => {
                            compute_pass.set_pipeline(&gpu.hash_fixed.pipeline);

                            let bind_group =
                                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: None,
                                    layout: &gpu.hash_fixed.bind_group_layout,
                                    entries: &[
                                        wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: runargs.fixed.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 1,
                                            resource: runargs.uniform.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 2,
                                            resource: runargs.output.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 3,
                                            resource: runargs.input.as_entire_binding(),
                                        },
                                    ],
                                });

                            // Set the bind group that we want to use
                            compute_pass.set_bind_group(0, &bind_group, &[]);

                            let workgroup_count = runargs.ops_len.div_ceil(workgroup_size as _);
                            compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
                        }
                        RunArgs::Variable(runargs) => {
                            compute_pass.set_pipeline(&gpu.hash_variable.pipeline);

                            todo!("hash_variable not implemented yet!");
                        }
                    }
                }
            }
        }

        debug!(
            "compute pass: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let output = prev_sb.unwrap()[0].0.clone();

        encoder.copy_buffer_to_buffer(&output, 0, &download, 0, output.size());

        let command_buffer = encoder.finish();

        debug!(
            "command buffer: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let si = gpu.queue.submit([command_buffer]);

        debug!(
            "submitted: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );

        Submission {
            device: gpu.device.clone(),
            si,
            download,
            debug,
            _download_convert: Default::default(),
        }
    }
}
