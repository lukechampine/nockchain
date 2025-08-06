use bytemuck::{Pod, Zeroable};
use nbx_tip5::melt::Melt;
use nockvm::noun::D;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferDescriptor, BufferUsages, ComputePass, CommandBuffer};
use zkvm_jetpack::form::bpoly::bitreverse;
use zkvm_jetpack::form::mary::{Mary, MarySlice};
use zkvm_jetpack::form::math::poly::p_ntt_twiddles;
use zkvm_jetpack::form::Belt;
use tracing::info_span;

use super::{FromBuffer, Gpu, Pipeline, Submission, Submittable, DebugHandle};
use crate::codewords::CodewordEngine;
use crate::gpu::get_gpu;
use crate::hash::{HashEngine, NounDigest};
use crate::instruments::local_instruments;
use crate::one::G;
use crate::utils::xeb;

pub struct CodewordResult {
    pub codeword_array: Mary,
    pub merk_heap: Mary,
}

pub struct CodewordMdata {
    pub codeword_step: u32,
    pub codeword_len: u32,
    pub mh_step: u32,
    pub mh_len: u32,
}

fn consume_to_mary(rev_bufs: &mut Vec<&[u8]>, step: u32, len: u32) -> Mary {
    let mut left = (step * len) as usize;

    let mut ma = Mary {
        step,
        len,
        dat: Vec::with_capacity(left),
    };

    while left > 0 {
        let b = rev_bufs.pop().unwrap();
        let (a, b) = b.split_at(core::cmp::min(b.len(), left * size_of::<u64>()));
        let a = bytemuck::cast_slice::<_, u64>(a);

        ma.dat.extend_from_slice(a);
        left -= a.len();

        if !b.is_empty() {
            rev_bufs.push(b);
        }
    }

    ma
}

impl FromBuffer for CodewordResult {
    type Metadata = CodewordMdata;

    fn from_buffers<T: AsRef<[u8]>>(b: &[T], mdata: &Self::Metadata) -> Self {
        let mut rev_b = b.iter().rev().map(|v| v.as_ref()).collect::<Vec<_>>();

        let codeword_array = consume_to_mary(&mut rev_b, mdata.codeword_step, mdata.codeword_len);

        let merk_heap = consume_to_mary(&mut rev_b, mdata.mh_step, mdata.mh_len);

        assert!(rev_b.is_empty());

        Self {
            codeword_array,
            merk_heap,
        }
    }
}

impl<'a> Submittable for CodewordEngine<'a> {
    type Output = CodewordResult;

    #[tracing::instrument(skip_all)]
    fn submit(self) -> Submission<Self::Output, CommandBuffer> {
        let (table_polys, fri_domain_len, total_cols) = self.destruct();

        let mh_height = xeb(fri_domain_len as usize);
        let mh_len: u32 = (1 << mh_height) - 1;

        let mdata = CodewordMdata {
            codeword_step: total_cols as u32,
            codeword_len: fri_domain_len,
            mh_step: 5,
            mh_len,
        };

        let gpu = get_gpu();
        let inst = local_instruments();
        let submit_probe = inst.gpu_submit_probe();

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let codewords = gpu.device.create_buffer(&BufferDescriptor {
            label: Some("codewords"),
            size: (fri_domain_len as u64) * total_cols * (size_of::<u64>() as u64),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // p_shift expects tail to be zeroed
        encoder.clear_buffer(&codewords, 0, Some(codewords.size()));

        // Single compute pass
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        // compute-lde
        compute_lde(
            &gpu, &mut compute_pass, table_polys, fri_domain_len, &codewords,
        );

        // mary-transpose
        let codeword_array = gpu.device.create_buffer(&BufferDescriptor {
            label: Some("codeword_array"),
            size: (fri_domain_len as u64) * total_cols * (size_of::<u64>() as u64),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        mary_transpose(
            &gpu, &mut compute_pass, &codewords, mdata.codeword_len, mdata.codeword_step, 1,
            &codeword_array,
        );

        // build-merk-heap
        let mh_bufs = build_merk_heap(
            &gpu, &mut compute_pass, &codeword_array, mdata.codeword_step,
        );

        core::mem::drop(compute_pass);

        let download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("download codewords")),
            size: codeword_array.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mh_download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("download merk-heap")),
            size: ((mh_len as usize) * size_of::<NounDigest<Belt>>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

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

        encoder.copy_buffer_to_buffer(&codeword_array, 0, &download, 0, download.size());

        let mut cur_off = 0;
        for src in mh_bufs.iter().rev() {
            encoder.copy_buffer_to_buffer(&src, 0, &mh_download, cur_off, src.size());
            cur_off += src.size();
        }
        assert_eq!(cur_off, mh_download.size());

        let command_buffer = encoder.finish();

        core::mem::drop(submit_probe);

        Submission {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            obj: command_buffer,
            downloads: vec![download, mh_download],
            debug,
            _download_convert: Default::default(),
            mdata,
        }
    }
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct BpShiftUniform {
    inp_offset: u32,
    inp_step: u32,
    num_elems: u32,
    out_offset: u32,
    out_step: u32,
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct BpNttUniform {
    off: u32,
    i: u32,
    poly_len: u32,
    num_elems: u32,
}

fn turn_coseword(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    polys: MarySlice,
    offset: Belt,
    order: u32,
    root: Belt,
    out: &Buffer,
    out_off: u64,
    out_len: u64,
) {
    let polys_buf = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("coseword-polys {}x{}", polys.step, polys.len)),
        contents: bytemuck::cast_slice(&polys.dat),
        usage: BufferUsages::STORAGE,
    });

    // Go through polys.step (polys) and order (out) chunks
    // First p_shift everything out

    // Precompute powers, so that we don't do it on GPU
    let mut pow = Belt(1);
    let powers = (0..polys.step)
        .map(|_| {
            let r = pow;
            pow *= offset;
            r
        })
        .collect::<Vec<_>>();

    let powers = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some("coseword-powers"),
        contents: bytemuck::cast_slice(&powers),
        usage: BufferUsages::STORAGE,
    });

    // src, src_step, dest, dest_step, powers (align chunks to step)
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size / polys.step) as usize;

    let pipeline = &gpu.bp_shift;

    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..(polys.len as usize)).step_by(chunk_size) {
        let num_chunks = core::cmp::min((polys.len as usize) - i, chunk_size);
        let num_elems = (num_chunks * (polys.step as usize)) as u32;

        let uniform = BpShiftUniform {
            inp_offset: (i as u32) * polys.step,
            inp_step: polys.step,
            num_elems,
            out_step: order,
            out_offset: out_off as u32 + (i as u32) * order,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("coseword-shift uniform {i}")),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: BufferUsages::UNIFORM,
        });

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
                    resource: polys_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: powers.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);

        let workgroup_count = num_elems.div_ceil(workgroup_size as _);
        //trace!("DISPATCH MUL {workgroup_count}");
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    // Then, p_ntt everything
    // Precompute swapping bitmask
    let mut swapmask = Vec::with_capacity(order as usize);
    let mut swapidx = Vec::with_capacity(order as usize);
    let log_2_of_n = order.ilog2();
    for k in 0..(order as usize) {
        let rk = bitreverse(k as u32, log_2_of_n) as usize;
        swapidx.push(rk as u32);
        if k < rk {
            swapmask.push(!0u64);
        } else {
            swapmask.push(0);
        }
    }

    let swapmask = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("coseword-swapmask {order}")),
        contents: bytemuck::cast_slice(&swapmask),
        usage: BufferUsages::STORAGE,
    });

    let swapidx = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("coseword-swapidx {order}")),
        contents: bytemuck::cast_slice(&swapidx),
        usage: BufferUsages::STORAGE,
    });

    // Swap all
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.bp_ntt_swap;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..(out_len as usize)).step_by(chunk_size) {
        let num_elems = core::cmp::min((out_len as usize) - i, chunk_size) as u32;

        let uniform = BpNttUniform {
            off: out_off as u32,
            i: i as u32,
            num_elems,
            poly_len: order,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("bp-ntt uniform {i}")),
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

    // Twiddle all
    let twiddles = p_ntt_twiddles(order as usize, &root);

    let pipeline = &gpu.bp_ntt;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let mut uniforms = vec![];

    for i in (0..((out_len / 2) as usize)).step_by(chunk_size) {
        let num_elems = core::cmp::min(((out_len / 2) as usize) - i, chunk_size) as u32;

        let uniform = BpNttUniform {
            off: (out_off as u32),
            i: i as u32,
            num_elems,
            poly_len: order,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("bp-ntt uniform {i}")),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: BufferUsages::UNIFORM,
        });

        let workgroup_count = num_elems.div_ceil(workgroup_size as _);

        uniforms.push((uniform, workgroup_count));
    }

    let pipeline = &gpu.bp_ntt;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for stage in 0..log_2_of_n {
        let twiddles = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("coseword-twiddles {} {stage}", polys.step)),
            contents: bytemuck::cast_slice(&twiddles[stage as usize]),
            usage: BufferUsages::STORAGE,
        });

        for (uniform, workgroup_count) in &uniforms {
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
                        resource: twiddles.as_entire_binding(),
                    },
                ],
            });

            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(*workgroup_count as u32, 1, 1);
        }
    }
}

fn compute_lde(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    table_polys: Vec<MarySlice>,
    fri_domain_len: u32,
    out: &Buffer,
) {
    let fri_domain_root = Belt(fri_domain_len as _).ordered_root().unwrap();

    let mut off = 0;

    for ma in table_polys {
        let split_len = fri_domain_len as u64 * ma.len as u64;
        turn_coseword(
            gpu,
            compute_pass,
            ma,
            G.into(),
            fri_domain_len,
            fri_domain_root,
            out,
            off / (size_of::<u64>() as u64),
            split_len,
        );
        off += split_len * (size_of::<u64>() as u64);
    }
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct MaryTransposeUniform {
    offset: u32,
    num_elems: u32,
    mary_step: u32,
    mary_len: u32,
    mary_off: u32,
}

fn mary_transpose(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    codewords: &Buffer,
    codeword_step: u32,
    codeword_len: u32,
    offset: u32,
    codeword_array: &Buffer,
) {
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.mary_transpose;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let mary_size = codeword_step * codeword_len * offset;
    assert_eq!(mary_size, (codeword_array.size() / 8) as u32);

    for i in (0..(mary_size as usize)).step_by(chunk_size) {
        let num_elems = core::cmp::min((mary_size as usize) - i, chunk_size) as u32;

        let uniform = MaryTransposeUniform {
            offset: i as u32,
            num_elems,
            mary_step: codeword_step,
            mary_len: codeword_len,
            mary_off: offset,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("mary-transpose uniform {i}")),
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
                    resource: codeword_array.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: codewords.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }
}

fn build_merk_heap(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    codeword_array: &Buffer,
    codeword_step: u32,
) -> Vec<Buffer> {
    // First, get the top-most layer (hashes of polys)
    let mut engine = HashEngine::default();
    engine.push_noun(0, D(1)).unwrap();
    engine.push_noun(0, D(codeword_step as u64)).unwrap();
    let hashes = engine.reduce_cpu();
    let step_hash = hashes[0];
    let len_hash = hashes[1];

    // hash (len (step poly))
    // So, we want to hash all polys into a buffer, then reduce the layer into hashes of (len,
    // poly), and finally, reduce to hash of (len, hash_steppoly)

    let codeword_melts = mont_all(
        gpu,
        compute_pass,
        &gpu.montify,
        codeword_array,
        BufferUsages::empty(),
    );

    let codewords_hashed = hash_varlen_multiple(gpu, compute_pass, &codeword_melts, codeword_step);
    let len_reduced = hash_10_fixedprepend(gpu, compute_pass, len_hash, &codewords_hashed);
    let step_reduced = hash_10_fixedprepend(gpu, compute_pass, step_hash, &len_reduced);

    let mut out = vec![step_reduced.clone()];

    // Then, reduce until one digest
    let mut cbuf = step_reduced;
    while cbuf.size() > (size_of::<NounDigest<Melt>>() as u64) {
        cbuf = hash_fixed_multiple(gpu, compute_pass, &cbuf);
        out.push(cbuf.clone());
    }

    // Finally, montyred everything
    out.into_iter()
        .map(|b| mont_all(gpu, compute_pass, &gpu.montyred, &b, BufferUsages::COPY_SRC))
        .collect()
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct MontUniform {
    offset: u32,
    num_elems: u32,
}

fn mont_all(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    pipeline: &Pipeline,
    input: &Buffer,
    extra_usage: BufferUsages,
) -> Buffer {
    let monted = gpu.device.create_buffer(&BufferDescriptor {
        label: Some("mont-all output"),
        size: input.size(),
        usage: BufferUsages::STORAGE | extra_usage,
        mapped_at_creation: false,
    });

    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    compute_pass.set_pipeline(&pipeline.pipeline);

    // TODO: use u64vec4
    let total_elems = (input.size() as usize) / size_of::<[u64; 1]>();

    for i in (0..total_elems).step_by(chunk_size) {
        let num_elems = core::cmp::min(total_elems - i, chunk_size) as u32;

        let uniform = MontUniform {
            offset: i as u32,
            num_elems,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("mont-all uniform {i}")),
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
                    resource: monted.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: input.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    monted
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct HashVarlenMultipleUniform {
    offset: u32,
    num_elems: u32,
    poly_len: u32,
}

fn hash_varlen_multiple(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    input: &Buffer,
    step: u32,
) -> Buffer {
    // We are reducing to 5 * (len / 8 / step) melts
    let num_melts = (input.size() as usize) / size_of::<u64>();
    let num_hashes = num_melts / (step as usize);

    let output = gpu.device.create_buffer(&BufferDescriptor {
        label: Some("hash-varlen-multiple"),
        size: (num_hashes * size_of::<NounDigest<Melt>>()) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.hash_varlen_multiple;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..num_hashes).step_by(chunk_size) {
        let num_elems = core::cmp::min(num_hashes - i, chunk_size) as u32;

        let uniform = HashVarlenMultipleUniform {
            offset: i as u32,
            num_elems,
            poly_len: step,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("hash-varlen-multiple uniform {i}")),
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
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: input.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    output
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct HashFixedMultipleUniform {
    offset: u32,
    num_elems: u32,
}

fn hash_fixed_multiple(gpu: &Gpu, compute_pass: &mut ComputePass, input: &Buffer) -> Buffer {
    // We are reducing to 5 * (len / 8 / step) melts
    let num_hashes = (input.size() as usize) / size_of::<NounDigest<Melt>>();

    let output = gpu.device.create_buffer(&BufferDescriptor {
        label: Some("hash-fixed-multiple"),
        size: input.size() / 2,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.hash_fixed_multiple;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..num_hashes).step_by(chunk_size) {
        let num_elems = core::cmp::min(num_hashes - i, chunk_size) as u32;

        let uniform = HashFixedMultipleUniform {
            offset: i as u32,
            num_elems,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("hash-fixed-multiple uniform {i}")),
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
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: input.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    output
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct Hash10FixedPrependUniform {
    offset: u32,
    num_elems: u32,
    prepend: NounDigest<Melt>,
}

fn hash_10_fixedprepend(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    prepend: NounDigest<Melt>,
    input: &Buffer,
) -> Buffer {
    // We are reducing to 5 * (len / 8 / step) melts
    let num_hashes = (input.size() as usize) / size_of::<NounDigest<Melt>>();

    let output = gpu.device.create_buffer(&BufferDescriptor {
        label: Some("hash-10-fixedprepend"),
        size: input.size(),
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.hash_10_fixedprepend;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..num_hashes).step_by(chunk_size) {
        let num_elems = core::cmp::min(num_hashes - i, chunk_size) as u32;

        let uniform = Hash10FixedPrependUniform {
            offset: i as u32,
            num_elems,
            prepend,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("hash-10-fixedprepend uniform {i}")),
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
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: input.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    output
}
