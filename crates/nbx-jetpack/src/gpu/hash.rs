use std::time::Instant;

use crate::log::*;
use wgpu::util::DeviceExt;
use wgpu::{Buffer, CommandBuffer};

use super::{get_gpu, FromBuffer, Submittable, WgOffsets, DebugHandle};
use crate::gpu::Submission;
use crate::instruments::local_instruments;
use crate::hash::{HashEngine, NounDigest, ReduceChunk};

impl FromBuffer for Vec<NounDigest> {
    type Metadata = ();

    fn from_buffers<T: AsRef<[u8]>>(b: &[T], _: &()) -> Self {
        let result: Vec<NounDigest> = b
            .iter()
            .map(|v| {
                let result: &[NounDigest] = bytemuck::cast_slice(v.as_ref());
                result
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        result
    }
}

#[derive(Debug)]
struct FixedRunArgs {
    input: Buffer,
    output: Buffer,
    fixed: Buffer,
    ops_len: usize,
    uniform: Buffer,
}

struct VariableRunArgs {
    input: Buffer,
    output: Buffer,
    variable: Buffer,
    ops_len: usize,
    uniform: Buffer,
}

impl Submittable for HashEngine {
    type Output = Vec<NounDigest>;

    #[tracing::instrument(skip_all)]
    fn submit(self) -> Submission<Self::Output, CommandBuffer> {
        let t = Instant::now();

        let gpu = get_gpu();
        let inst = local_instruments();
        let submit_probe = inst.gpu_submit_probe();

        let (mut stages, out_stages) = self.destruct();

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
        let mut download_size = 0;
        let mut buffers_to_download = vec![];

        for (i, s) in stages.iter().enumerate() {
            let mut usage = wgpu::BufferUsages::STORAGE;

            if i < out_stages {
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
                trace!(
                    "generating stage {i}-{o}: output_len={}, fixed_len={}, variable_len={}",
                    out.len(),
                    ops_fixed.len(),
                    ops_variable.len()
                );

                let output = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("out {i}-{o}")),
                        contents: bytemuck::cast_slice(&out),
                        usage,
                    });

                if i < out_stages {
                    buffers_to_download.push((output.clone(), download_size));
                    download_size += output.size();
                }

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

            let mut fixed_run_args = vec![];
            let mut variable_run_args = vec![];

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
                let variable_off =
                    unsafe { chunk.ops_variable.as_ptr().offset_from(chunks[chunk_idx].2) };

                // TODO: pad / extend var_off

                if let Some(psb) = &prev_sb {
                    let (output, fixed, variable) = &sb[chunk_idx];

                    // Add fixed chunks
                    for (chunk_idx, c) in chunk.ops_fixed.chunks(chunk_size).enumerate() {
                        trace!("Adding fixed substage");
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

                        fixed_run_args.push(FixedRunArgs {
                            input: psb[inp_idx].0.clone(),
                            output: output.clone(),
                            fixed: fixed.clone(),
                            ops_len: c.len(),
                            uniform,
                        });
                    }

                    // Add variable chunks
                    for (chunk_idx, c) in chunk.ops_variable.chunks(chunk_size).enumerate() {
                        trace!("Adding variable substage");
                        let offs = WgOffsets {
                            ops: ((chunk_idx * chunk_size) + variable_off as usize) as _,
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

                        variable_run_args.push(VariableRunArgs {
                            input: psb[inp_idx].0.clone(),
                            output: output.clone(),
                            variable: variable.clone(),
                            ops_len: c.len(),
                            uniform,
                        });
                    }
                } else {
                    unreachable!();
                }
            }

            processed_stages.push((fixed_run_args, variable_run_args));
            prev_sb = Some(sb);
            cur_inputs = stage.chunks.into_iter().map(|v| v.out).collect::<Vec<_>>();
        }

        debug!(
            "runs: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: download_size,
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

        let debug: DebugHandle = if debug_capture == Some(true) {
            unsafe { gpu.device.start_graphics_debugger_capture() };
            debug_capture_guard.unwrap().take();
            Some(gpu.debug_capture.clone())
        } else {
            core::mem::drop(debug_capture_guard);
            None
        }.into();

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            // Single compute pass
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });

            for (fixed_runs, variable_runs) in processed_stages {
                compute_pass.set_pipeline(&gpu.hash_fixed.pipeline);
                for runargs in fixed_runs {
                    let bind_group =
                        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: None,
                            layout: &gpu.hash_fixed.bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: runargs.uniform.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: runargs.fixed.as_entire_binding(),
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

                compute_pass.set_pipeline(&gpu.hash_variable.pipeline);
                for runargs in variable_runs {
                    let bind_group =
                        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: None,
                            layout: &gpu.hash_variable.bind_group_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: runargs.uniform.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: runargs.variable.as_entire_binding(),
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
            }
        }

        debug!(
            "compute pass: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        for (buf, off) in buffers_to_download {
            encoder.copy_buffer_to_buffer(&buf, 0, &download, off, buf.size());
        }

        let command_buffer = encoder.finish();

        debug!(
            "command buffer: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        core::mem::drop(submit_probe);

        debug!(
            "submitted: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );

        Submission {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            obj: command_buffer,
            downloads: vec![download],
            debug,
            _download_convert: Default::default(),
            mdata: (),
        }
    }
}
