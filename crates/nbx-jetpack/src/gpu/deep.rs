use bytemuck::{Pod, Zeroable};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferDescriptor, BufferUsages, CommandBuffer, ComputePass};
use zkvm_jetpack::form::{Belt, FPolyVec, Felt, PolyVec};

use super::substitute::AccumUniform;
use super::{get_gpu, DebugHandle, FromBuffer, Gpu, Submission, Submittable};
use crate::deep::{DeepEngine, LinearCombo, WeightedDivConst};
use crate::gpu::util::p_ntt;
use crate::instruments::local_instruments;

pub struct DeepResult {
    pub res: FPolyVec,
}

impl FromBuffer for DeepResult {
    type Metadata = ();

    fn from_buffers<T: AsRef<[u8]>>(b: &[T], _: &Self::Metadata) -> Self {
        assert_eq!(b.len(), 1);

        let b = b[0].as_ref();

        DeepResult {
            res: PolyVec(bytemuck::cast_slice(b).to_vec()),
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct WeightedDivConstUniform {
    dq: u32,
    deg_prod: u32,
    d: Felt,
    inv_len: Felt,
}

impl<'a> Submittable for DeepEngine<'a> {
    type Output = DeepResult;

    #[tracing::instrument(skip_all)]
    fn submit(self) -> Submission<Self::Output, CommandBuffer> {
        let (combos, weights) = self.destruct();

        let gpu = get_gpu();
        let inst = local_instruments();
        let submit_probe = inst.gpu_submit_probe();

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let weights = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("compute-deep-weights"),
            contents: bytemuck::cast_slice(weights.0),
            usage: BufferUsages::STORAGE,
        });

        let max_dq = combos.iter().map(|v| v.id_x.dq + 1).max().unwrap_or(1);
        let accum_out = gpu.device.create_buffer(&BufferDescriptor {
            label: Some("compute-deep-acc"),
            size: (max_dq * size_of::<Felt>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        encoder.clear_buffer(&accum_out, 0, Some(accum_out.size()));

        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        for (i, combo) in combos.into_iter().enumerate() {
            weighted_linear_combo(&gpu, &mut compute_pass, i, combo, &weights, &accum_out);
        }

        core::mem::drop(compute_pass);

        let download = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("download acc")),
            size: accum_out.size(),
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
        }
        .into();

        encoder.copy_buffer_to_buffer(&accum_out, 0, &download, 0, download.size());

        let command_buffer = encoder.finish();
        core::mem::drop(submit_probe);

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

fn weighted_linear_combo(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    i: usize,
    combo: LinearCombo,
    weights: &Buffer,
    accum_out: &Buffer,
) {
    let LinearCombo {
        id_x,
        lead_felts,
        rf_polys,
        weights_off,
    } = combo;

    /*let id_x_uniform = WeightedDivConstUniform {
        dq: id_x.dq as u32,
        deg_prod: id_x.deg_prod as u32,
        d: id_x.d,
        inv_len: id_x.inv_len,
    };*/

    /*let id_x_uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("linear-combo id-x-uniform {i}")),
        contents: bytemuck::cast_slice(&[id_x_uniform]),
        usage: BufferUsages::UNIFORM,
    });*/
    let pinned_ntt = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("linear-combo pinned-ntt {i}")),
        contents: bytemuck::cast_slice(&id_x.pinned_ntt.0),
        usage: BufferUsages::STORAGE,
    });
    let twiddles = id_x
        .twiddles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            gpu.device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("linear-combo twiddles {i}")),
                contents: bytemuck::cast_slice(&t),
                usage: BufferUsages::STORAGE,
            })
        })
        .collect::<Vec<_>>();
    let ifft_twiddles = id_x
        .ifft_twiddles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            gpu.device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("linear-combo ifft-twiddles {i}")),
                contents: bytemuck::cast_slice(&t),
                usage: BufferUsages::STORAGE,
            })
        })
        .collect::<Vec<_>>();
    let lead_felts = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("linear-combo lead-felts {i}")),
        contents: bytemuck::cast_slice(&lead_felts),
        usage: BufferUsages::STORAGE,
    });
    let rf_polys = gpu.device.create_buffer_init(&BufferInitDescriptor {
        label: Some(&format!("linear-combo rf-polys {i}")),
        contents: bytemuck::cast_slice(&rf_polys),
        usage: BufferUsages::STORAGE,
    });

    let res = weighted_combo(
        gpu, compute_pass, id_x.deg_prod, &lead_felts, &rf_polys, &id_x, &pinned_ntt, &twiddles,
        &ifft_twiddles, weights, weights_off,
    );

    //accum everyrhing
    let out_poly_len = id_x.dq + 1;
    let out_poly_size = size_of::<Felt>() * out_poly_len;
    let remaining_polys_raw = (res.size() as usize) / out_poly_size;
    let mut remaining_polys = 1 << remaining_polys_raw.ilog2();

    // Move the tail
    if remaining_polys_raw != remaining_polys {
        fp_accum(
            gpu,
            compute_pass,
            &res,
            0,
            remaining_polys as u32,
            remaining_polys_raw - remaining_polys,
            out_poly_len,
            &res,
        );
    }

    while remaining_polys > 1 {
        let new_remaining_polys = remaining_polys / 2;
        fp_accum(
            gpu, compute_pass, &res, 0, new_remaining_polys as u32, new_remaining_polys,
            out_poly_len, &res,
        );
        remaining_polys = new_remaining_polys;
    }

    fp_accum(gpu, compute_pass, &res, 0, 0, 1, out_poly_len, &accum_out);
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct FpAccumUniform {
    i: u32,
    num_elems: u32,
    inp_off1: u32,
    inp_off2: u32,
}

fn fp_accum(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    inp: &Buffer,
    inp_poly1: u32,
    inp_poly2: u32,
    num_polys_out: usize,
    poly_len: usize,
    out: &Buffer,
) {
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    //let out_len = (inp.size() as usize) / size_of::<u64>() / 2;
    let out_len = num_polys_out * poly_len;
    let inp_off1 = inp_poly1 * (poly_len as u32);
    let inp_off2 = inp_poly2 * (poly_len as u32);

    let pipeline = &gpu.fp_accum;
    compute_pass.set_pipeline(&pipeline.pipeline);

    for i in (0..out_len).step_by(chunk_size) {
        let num_elems = core::cmp::min(out_len - i, chunk_size) as u32;
        let offs = FpAccumUniform {
            i: i as u32,
            num_elems,
            inp_off1,
            inp_off2,
        };

        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("fp-accum {out_len}-{i}")),
                contents: bytemuck::cast_slice(&[offs]),
                usage: wgpu::BufferUsages::UNIFORM,
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
                    resource: inp.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct WeightedComboFinishUniform {
    inv_len: [Belt; 3],
    i: u32,
    num_elems: u32,
    in_poly_len: u32,
    out_poly_len: u32,
    weights_off: u32,
    _pad: u32,
}

fn weighted_combo(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    stride: usize,
    lead_felts: &Buffer,
    rf_polys: &Buffer,
    id_x: &WeightedDivConst,
    pinned_ntt: &Buffer,
    twiddles: &[Buffer],
    ifft_twiddles: &[Buffer],
    weights: &Buffer,
    weights_off: usize,
) -> Buffer {
    // NTT with twiddles
    let out_len = (rf_polys.size() as usize) / size_of::<Felt>();
    p_ntt(
        gpu, compute_pass, stride as u32, rf_polys, 0, out_len as u64, twiddles, 3, &gpu.fp_ntt,
    );

    // hadamard
    fp_hadamard_samepoly(gpu, compute_pass, stride as u32, rf_polys, pinned_ntt);

    // NTT with ifft_twiddles
    p_ntt(
        gpu, compute_pass, stride as u32, rf_polys, 0, out_len as u64, ifft_twiddles, 3,
        &gpu.fp_ntt,
    );

    // scag + rev + scal + scal
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.weighted_combo_finish;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let out_poly = gpu.device.create_buffer(&BufferDescriptor {
        label: Some("weighted-combo-finish out"),
        size: ((out_len / stride) * (id_x.dq + 1) * size_of::<Felt>()) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    for i in (0..out_len).step_by(chunk_size) {
        let num_elems = core::cmp::min(out_len - i, chunk_size) as u32;
        let uniform = WeightedComboFinishUniform {
            inv_len: id_x.inv_len.0,
            i: i as u32,
            num_elems,
            in_poly_len: stride as u32,
            out_poly_len: (id_x.dq + 1) as u32,
            weights_off: weights_off as u32,
            _pad: 0,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("weighted-combo-finish uniform {i}")),
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
                    resource: out_poly.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: rf_polys.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: lead_felts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: weights.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }

    out_poly
}
//    let mulled = fpmul_fast_cached(pinned_ntt.into(), ifft_twiddles, twiddles, rf);
//    let mut scagged = PolyVec(dq + 1, mulled.0));
//    scagged.0.reverse();
//    pscal_inplace(lead * *inv_len, &mut scagged.0);
//    scagged
//}
//
//// ::  +fpmul-fast: polynomial multiplication with fft
//fn fpmul_fast_cached<'a>(
//    a: FPolySlice,
//    ifft_twiddles: &[impl AsRef<[Felt]>],
//    twiddles: &[impl AsRef<[Felt]>],
//    b: FPolyVec,
//) -> FPolyVec {
//    let mut b = PolyVec(p_ntt_twiddled(b.0, &twiddles));
//    p_hadamard_inplace(&mut b.0, &a.0);
//    let ntt = p_ntt_twiddled(b.0, ifft_twiddles);
//    PolyVec(ntt)

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct FpHadamardSamepolyUniform {
    i: u32,
    poly_len: u32,
    num_elems: u32,
}

fn fp_hadamard_samepoly(
    gpu: &Gpu,
    compute_pass: &mut ComputePass,
    poly_len: u32,
    polys: &Buffer,
    fixed_poly: &Buffer,
) {
    // src, src_step, dest, dest_step, powers (align chunks to step)
    let workgroup_size = 256;
    let max_workgroup_insts = 65535;
    let chunk_size = (max_workgroup_insts * workgroup_size) as usize;

    let pipeline = &gpu.fp_hadamard_samepoly;
    compute_pass.set_pipeline(&pipeline.pipeline);

    let out_len: usize = (polys.size() as usize) / size_of::<Felt>();

    for i in (0..out_len).step_by(chunk_size) {
        let num_elems = core::cmp::min(out_len - i, chunk_size) as u32;
        let uniform = FpHadamardSamepolyUniform {
            i: i as u32,
            num_elems,
            poly_len,
        };

        let uniform = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!("fp-hadamard-samepoly uniform {i}")),
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
                    resource: polys.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: fixed_poly.as_entire_binding(),
                },
            ],
        });

        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
    }
}
