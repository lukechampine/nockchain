use std::collections::BTreeMap;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use nbx_tip5::melt::Melt;
use tracing::{info_span, *};
use wgpu::util::DeviceExt;
use wgpu::Buffer;

use super::{get_gpu, FromBuffer, Submittable};
use crate::gpu::Submission;
use crate::substitute::{SubstituteEngine, MAX_CHUNK_SIZE};

#[derive(Clone, Copy, Pod, Zeroable, Debug)]
#[repr(C)]
pub struct SubstituteIterOps {
    pub scal: Melt,
    pub vars: u32,
    pub num_vars: u16,
    pub iter_id: u16,
}

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(super) struct MulUniform {
    poly_len: u32,
    chunks_per_buf: u32,
    ops: u32,
    num_ops: u32,
    inp: u32,
    accum_mask: u32,
    out: u32,
}

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub(super) struct AccumUniform {
    inp_a: u32,
    inp_b: u32,
    num_elems: u32,
    out: u32,
    out_mask: u32,
}

impl FromBuffer for Vec<Vec<Melt>> {
    fn from_buffers<T: AsRef<[u8]>>(b: &[T]) -> Self {
        b.iter()
            .map(|v| bytemuck::cast_slice::<_, Melt>(v.as_ref()).to_vec())
            .collect()
    }
}

impl<'a> Submittable for SubstituteEngine<'a, Melt> {
    type Output = Vec<Vec<Melt>>;

    #[tracing::instrument(skip_all)]
    fn submit(self) -> Submission<Self::Output> {
        let t = Instant::now();

        let gpu = get_gpu();

        let (mut stages, poly_len) = self.destruct();

        let workgroup_size = 256;
        let max_workgroup_insts = 65535;
        let shader_stride = 2;
        let poly_strides = poly_len / shader_stride;

        let chunks_per_buf = MAX_CHUNK_SIZE / poly_len;

        let chunk_size = core::cmp::min(
            max_workgroup_insts * workgroup_size,
            chunks_per_buf * poly_len,
        );

        // First, allocate buffers for all outputs
        let mut stage_buffers = vec![];
        let mut traces: BTreeMap<(*const Melt, usize), Buffer> = BTreeMap::new();

        info_span!("stage_buffers").in_scope(|| {
            for (i, s) in stages.iter().enumerate() {
                trace!("stage {i}");

                let mut usage = wgpu::BufferUsages::STORAGE;

                if i == 0 {
                    usage |= wgpu::BufferUsages::COPY_SRC;
                }

                let mut out_bufs = vec![];
                info_span!("output").in_scope(|| {
                    for (o, out) in s.out.iter().enumerate() {
                        trace!("output {i}-{o}");
                        let output =
                            gpu.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some(&format!("out {i}-{o}")),
                                    contents: bytemuck::cast_slice(&out),
                                    usage,
                                });
                        out_bufs.push(output);
                    }
                });

                let mut iter_buffers = vec![];

                info_span!("iter_buffers").in_scope(|| {
                    for (o, iter) in s.iters.iter().enumerate() {
                        trace!("iter {i}-{o}");
                        let traces = info_span!("traces").in_scope(|| {
                            let t = iter.traces.0;
                            traces
                                .entry((t.as_ptr(), t.len()))
                                .or_insert_with(|| {
                                    gpu.device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some(&format!("traces {i}-{o}")),
                                            contents: bytemuck::cast_slice(t),
                                            usage: wgpu::BufferUsages::STORAGE,
                                        },
                                    )
                                })
                                .clone()
                        });

                        info_span!("post_traces").in_scope(|| {
                            let mut ops_map: BTreeMap<Option<_>, Vec<_>> = BTreeMap::new();
                            let mut buf_ids = vec![];
                            let mut subs = vec![];
                            let mut mul_splits = vec![];

                            info_span!("muls").in_scope(|| {
                                for (u, mul) in iter.muls.iter().enumerate() {
                                    //trace!("mul stage");
                                    let mut cur_splits = BTreeMap::new();
                                    // Trace indices
                                    {
                                        let mut var_ops = mul.vars.clone();
                                        var_ops.sort_by_key(|v| v.chunk);
                                        let iter_ops = SubstituteIterOps {
                                            scal: mul.scal,
                                            vars: subs.len() as u32,
                                            num_vars: var_ops.len() as u16,
                                            iter_id: u as _,
                                        };
                                        buf_ids.push((None, var_ops.len()));
                                        //trace!("vars: {var_ops:?}");
                                        subs.extend(var_ops);
                                        let e = ops_map.entry(None).or_default();
                                        cur_splits.insert(None, e.len());
                                        e.push(iter_ops);
                                    }

                                    // Prev out indices
                                    // We need to split these ops up by appropriate buffer ID
                                    let mut com_ops = mul.coms.clone();
                                    com_ops.sort_by_key(|v| v.chunk);
                                    let mut prev_buf = None;
                                    for com_ops in com_ops.split(|v| {
                                        let buf = v.chunk as usize / chunks_per_buf;
                                        if prev_buf.is_none() {
                                            prev_buf = Some(buf);
                                        }
                                        if prev_buf != Some(buf) {
                                            prev_buf = Some(buf);
                                            true
                                        } else {
                                            false
                                        }
                                    }) {
                                        if com_ops.is_empty() {
                                            continue;
                                        }
                                        let buf = com_ops[0].chunk as usize / chunks_per_buf;
                                        let iter_ops = SubstituteIterOps {
                                            scal: Melt::from_u64(1),
                                            vars: subs.len() as u32,
                                            num_vars: com_ops.len() as u16,
                                            iter_id: u as _,
                                        };
                                        buf_ids.push((Some(buf), com_ops.len()));
                                        //trace!("coms: {com_ops:?}");
                                        subs.extend(com_ops);
                                        let e = ops_map.entry(Some(buf)).or_default();
                                        cur_splits.insert(Some(buf), e.len());
                                        e.push(iter_ops);
                                    }

                                    mul_splits.push(cur_splits);
                                }
                            });

                            // Flatten the ops out to be in the order of bufs
                            let mut ops = vec![];
                            let mut ops_offsets: BTreeMap<Option<_>, usize> = BTreeMap::new();
                            let mut cur_offset = 0;
                            for (bid, v) in ops_map {
                                ops_offsets.insert(bid, cur_offset);
                                cur_offset += v.len();
                                ops.extend(v);
                            }

                            trace!("ALL OPS {}", ops.len());
                            trace!("ALL SUBS {}", subs.len());

                            let ops_buf =
                                gpu.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some(&format!("ops {i}-{o}")),
                                        contents: bytemuck::cast_slice(&ops),
                                        usage: wgpu::BufferUsages::STORAGE,
                                    });

                            let subs =
                                gpu.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some(&format!("subs {i}-{o}")),
                                        contents: bytemuck::cast_slice(&subs),
                                        usage: wgpu::BufferUsages::STORAGE,
                                    });

                            iter_buffers.push((
                                traces, ops, ops_offsets, buf_ids, ops_buf, subs, mul_splits,
                            ));
                        });
                    }
                });

                stage_buffers.push((out_bufs, iter_buffers));
            }
        });

        let mul_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("mulaccum")),
            size: (chunks_per_buf * poly_len * 8) as _,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        debug!("stage buffers: {:.02}", t.elapsed().as_secs_f64());
        let t2 = Instant::now();

        let mut prev_ob: Option<Vec<Buffer>> = None;
        let mut processed_stages = vec![];
        let mut cur_stage = 0;

        info_span!("process_stages").in_scope(|| {
            while let Some(stage) = stages.pop() {
                trace!("stage {cur_stage}");
                let (ob, ib) = stage_buffers.pop().unwrap();

                let mut stage_muls = vec![];
                let mut stage_accums = vec![];

                for ((i, iter), (traces, ops, ops_offsets, buf_ids, ops_buf, subs, mul_splits)) in
                    stage.iters.into_iter().enumerate().zip(ib)
                {
                    // First, dispatch multiplications into mul bufs (or output)
                    trace!("iter {i}");
                    let mut iter_muls = vec![];
                    let mut iter_accums = vec![];

                    let final_chunk = &ob[i / chunks_per_buf];
                    let final_offset = (i % chunks_per_buf) * poly_strides; // * core::mem::size_of::<Melt>();

                    for (o, split_chunk) in mul_splits.chunks(chunk_size / poly_len).enumerate() {
                        let (output, offset) = if split_chunk.len() > 1 {
                            (mul_buf.clone(), 0)
                        } else {
                            (final_chunk.clone(), final_offset)
                        };

                        let mut chunk_muls = vec![];
                        let mut chunk_accums = vec![];
                        trace!("split_chunk {o} {}", split_chunk.len());
                        for (buf_id, input) in [(None, &traces)].into_iter().chain(
                            prev_ob
                                .iter()
                                .flatten()
                                .enumerate()
                                .map(|(a, b)| (Some(a), b)),
                        ) {
                            trace!("bufid: {buf_id:?}");

                            let Some(ops_off) = ops_offsets.get(&buf_id) else {
                                continue;
                            };

                            // Go through split_chunk, look at the map, collect all entries that have
                            // current buf_id. Just count the number of them.
                            let Some(&ops_start) =
                                split_chunk.iter().filter_map(|v| v.get(&buf_id)).next()
                            else {
                                error!("No ops_start, even though we have ops_off");
                                continue;
                            };
                            let Some(&ops_end) = split_chunk
                                .iter()
                                .rev()
                                .filter_map(|v| v.get(&buf_id))
                                .next()
                            else {
                                error!("No ops_end, even though we had ops_start");
                                continue;
                            };

                            let num_ops = ops_end - ops_start + 1;
                            trace!("num_ops {num_ops}");

                            let offs = MulUniform {
                                poly_len: poly_len as _,
                                chunks_per_buf: chunks_per_buf as _,
                                ops: (ops_start + ops_off) as _,
                                num_ops: num_ops as _,
                                inp: 0,
                                accum_mask: if buf_id.is_none() { 0 } else { !0 },
                                out: offset as _,
                            };

                            let uniform =
                                gpu.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some(&format!(
                                            "mul {cur_stage}-{i}-{o} -- {:x}",
                                            buf_id.unwrap_or(!0)
                                        )),
                                        contents: bytemuck::cast_slice(&[offs]),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    });

                            chunk_muls.push((
                                (offs.num_ops as usize) * poly_strides,
                                input.clone(),
                                output.clone(),
                                ops_buf.clone(),
                                subs.clone(),
                                uniform,
                            ));
                        }

                        // Then, dispatch accumulation steps, while reusing the buffers
                        if split_chunk.len() > 1 {
                            let mut accum_ops = split_chunk.len();
                            let mut accum_iter = 0;
                            while accum_ops > 2 {
                                trace!("treeaccum {accum_ops}");
                                let right_ops = (accum_ops + 1) / 2;
                                let num_ops = accum_ops / 2;

                                let offs = AccumUniform {
                                    inp_a: 0,
                                    inp_b: (poly_strides * right_ops) as _,
                                    num_elems: (poly_strides * num_ops) as _,
                                    out: 0 as _,
                                    out_mask: 0,
                                };

                                let uniform = gpu.device.create_buffer_init(
                                    &wgpu::util::BufferInitDescriptor {
                                        label: Some(&format!(
                                            "accum {cur_stage}-{i}-{o}-{accum_iter}"
                                        )),
                                        contents: bytemuck::cast_slice(&[offs]),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    },
                                );

                                chunk_accums.push((
                                    offs.num_elems as usize,
                                    mul_buf.clone(),
                                    mul_buf.clone(),
                                    uniform,
                                ));
                                accum_iter += 1;
                                accum_ops = right_ops;
                            }

                            assert_eq!(accum_ops, 2);
                            trace!("finalaccum {poly_strides}");

                            let offs = AccumUniform {
                                inp_a: 0,
                                inp_b: poly_strides as _,
                                num_elems: poly_strides as _,
                                out: final_offset as _,
                                out_mask: !0,
                            };

                            let uniform =
                                gpu.device
                                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                        label: Some(&format!("accum {i}-{o}-FINAL")),
                                        contents: bytemuck::cast_slice(&[offs]),
                                        usage: wgpu::BufferUsages::UNIFORM,
                                    });

                            chunk_accums.push((
                                offs.num_elems as usize,
                                mul_buf.clone(),
                                final_chunk.clone(),
                                uniform,
                            ));
                        }

                        iter_muls.push(chunk_muls);
                        iter_accums.push(chunk_accums);
                    }

                    stage_muls.push(iter_muls);
                    stage_accums.push(iter_accums);
                }

                trace!("ps: {} {}", stage_muls.len(), stage_accums.len());

                processed_stages.push((stage_muls, stage_accums));
                prev_ob = Some(ob);
                cur_stage += 1;
            }
        });

        debug!(
            "runs: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        info_span!("encode_commands").in_scope(|| {
            for (stage_muls, stage_accums) in processed_stages {
                for (chunk_muls, chunk_accums) in stage_muls
                    .into_iter()
                    .flatten()
                    .zip(stage_accums.into_iter().flatten())
                {
                    // ALL combined multiplications+accums to be done for this ITER.
                    {
                        let pipeline = &gpu.substitute_mul;

                        // Single compute pass
                        let mut compute_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: None,
                                timestamp_writes: None,
                            });

                        // Set the pipeline that we want to use
                        compute_pass.set_pipeline(&pipeline.pipeline);

                        // Here, bind the correct buffers based on
                        for (num_ops, input, output, ops, subs, uniform) in chunk_muls {
                            let bind_group =
                                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: None,
                                    layout: &pipeline.bind_group_layout,
                                    entries: &[
                                        wgpu::BindGroupEntry {
                                            binding: 0,
                                            // SubstituteIterOps
                                            resource: ops.as_entire_binding(),
                                        },
                                        // Contains the starting offset within the SubstituteIterOps
                                        // The shader offsets with gl_WorkGroupID * gl_WorkGroupSize / poly_len
                                        // to access the right operation.
                                        wgpu::BindGroupEntry {
                                            binding: 1,
                                            resource: uniform.as_entire_binding(),
                                        },
                                        // Output buffer - get from bufs, or go directly to main output, if
                                        // we have only one set of muls.
                                        wgpu::BindGroupEntry {
                                            binding: 2,
                                            resource: output.as_entire_binding(),
                                        },
                                        // SubstituteOp - contains all multiplications to be done.
                                        // We start with taking the scal from the SubstituteIterOps, and
                                        // then applying the operations one op at a time. (or, if we're
                                        // daring, 4 ops at a time (then the previous offset needs to
                                        // take that into account)).
                                        wgpu::BindGroupEntry {
                                            binding: 3,
                                            resource: subs.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 4,
                                            resource: input.as_entire_binding(),
                                        },
                                    ],
                                });

                            // Set the bind group that we want to use
                            compute_pass.set_bind_group(0, &bind_group, &[]);

                            let workgroup_count = num_ops.div_ceil(workgroup_size as _);
                            trace!("DISPATCH MUL {workgroup_count}");
                            compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
                        }
                    }

                    // ALL accumulations to be done for this ITER.
                    {
                        let pipeline = &gpu.substitute_accum;

                        // Single compute pass
                        let mut compute_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: None,
                                timestamp_writes: None,
                            });

                        // Set the pipeline that we want to use
                        compute_pass.set_pipeline(&pipeline.pipeline);

                        // Now, do a series of accumulations
                        // From ops.len() and bufs.len() reconstruct the buffers we want to use,
                        // and perform the accumulations correctly.
                        //stage_accums.push((inp_a, inp_b, final_chunk, uniform));
                        for (num_accoms, inp, output, uniform) in chunk_accums {
                            let bind_group =
                                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: None,
                                    layout: &pipeline.bind_group_layout,
                                    entries: &[
                                        // Contains the starting offsets within the buffers.
                                        // offsets with gl_GlobalInvocationId
                                        wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: uniform.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 1,
                                            resource: output.as_entire_binding(),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 2,
                                            resource: inp.as_entire_binding(),
                                        },
                                    ],
                                });

                            // Set the bind group that we want to use
                            compute_pass.set_bind_group(0, &bind_group, &[]);

                            let workgroup_count = num_accoms.div_ceil(workgroup_size as _);
                            trace!("DISPATCH ACCUM {workgroup_count}");
                            compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
                        }
                    }
                }
            }
        });

        debug!(
            "compute pass: {:.02}, {:.02}",
            t.elapsed().as_secs_f64(),
            t2.elapsed().as_secs_f64()
        );
        let t2 = Instant::now();

        info_span!("finish_up").in_scope(|| {
            let downloads = prev_ob
                .iter()
                .flatten()
                .enumerate()
                .map(|(i, v)| {
                    gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("download {i}")),
                        size: v.size(),
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    })
                })
                .collect::<Vec<_>>();

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

            for (download, output) in downloads.iter().zip(prev_ob.iter().flatten()) {
                encoder.copy_buffer_to_buffer(&output, 0, &download, 0, output.size());
            }

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
                downloads,
                debug,
                _download_convert: Default::default(),
            }
        })
    }
}
