use bytemuck::{Pod, Zeroable};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, ComputePass};
use zkvm_jetpack::form::bpoly::bitreverse;

use super::{Gpu, Pipeline};

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct PNttUniform {
    off: u32,
    i: u32,
    poly_len: u32,
    num_elems: u32,
}

pub fn p_ntt(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    order: u32,
    out: &Buffer,
    out_off: u64,
    out_len: u64,
    twiddles: &[Buffer],
    elem_len: usize,
    elem_ntt_pipeline: &Pipeline,
) {
    // src, src_step, dest, dest_step, powers (align chunks to step)
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;

    // Then, p_ntt everything
    // Precompute swapping bitmask
    let mut swapmask = Vec::with_capacity(order as usize * elem_len);
    let mut swapidx = Vec::with_capacity(order as usize * elem_len);
    let log_2_of_n = order.ilog2();
    for k in 0..(order as usize) {
        let rk = bitreverse(k as u32, log_2_of_n) as usize;
        for i in 0..elem_len {
            swapidx.push((rk * elem_len + i) as u32);
            if k < rk {
                swapmask.push(!0u64);
            } else {
                swapmask.push(0);
            }
        }
    }

    let swapmask = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("ntt-swapmask {order}")),
        contents: bytemuck::cast_slice(&swapmask),
        usage: BufferUsages::STORAGE,
    });

    let swapidx = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("ntt-swapidx {order}")),
        contents: bytemuck::cast_slice(&swapidx),
        usage: BufferUsages::STORAGE,
    });

    // Swap all
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.p_ntt_swap;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let out_len_swap = (out_len as usize) * elem_len;

    for i in (0..out_len_swap).step_by(chunk_size) {
        let num_elems = core::cmp::min(out_len_swap - i, chunk_size) as u32;

        let uniform = PNttUniform {
            off: out_off as u32,
            i: i as u32,
            num_elems,
            poly_len: order * (elem_len as u32),
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("ntt-swap uniform {i}")),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: BufferUsages::UNIFORM,
        });

        let workgroup_count = num_elems.div_ceil(workgroup_size as _);

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: swapmask.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: swapidx.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    let pipeline = &gpu.bp_ntt;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let mut uniforms = vec![];

    for i in (0..((out_len / 2) as usize)).step_by(chunk_size) {
        let num_elems = core::cmp::min(((out_len / 2) as usize) - i, chunk_size) as u32;

        let uniform = PNttUniform {
            off: (out_off as u32),
            i: i as u32,
            num_elems,
            poly_len: order,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("ntt uniform {i}")),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: BufferUsages::UNIFORM,
        });

        let workgroup_count = num_elems.div_ceil(workgroup_size as _);

        uniforms.push((uniform, workgroup_count));
    }

    compute_pass.set_pipeline(&elem_ntt_pipeline.pipeline);

    for stage in 0..log_2_of_n {
        let twiddles = &twiddles[stage as usize];

        for (uniform, workgroup_count) in &uniforms {
            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &elem_ntt_pipeline.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: out.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: twiddles.as_entire_binding(),
                    },
                ],
            });

            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(*workgroup_count as u32, 1, 1);
        }
    }
}
