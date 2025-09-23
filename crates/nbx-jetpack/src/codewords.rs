use zkvm_jetpack::form::mary::{Mary, MarySlice};
use zkvm_jetpack::form::math::mary::mary_transpose;
use zkvm_jetpack::form::Belt;

use crate::eight::compute_lde;
use crate::engine::Engine;
use crate::three::{build_merk_heap_impl, MerkHeap};

#[derive(Clone)]
pub struct CodewordEngine<'a> {
    table_polys: Vec<MarySlice<'a>>,
    fri_domain_len: u32,
    total_cols: u64,
}

impl<'a> CodewordEngine<'a> {
    pub fn new(table_polys: Vec<MarySlice<'a>>, fri_domain_len: u32, total_cols: u64) -> Self {
        Self {
            table_polys,
            fri_domain_len,
            total_cols,
        }
    }

    pub fn destruct(self) -> (Vec<MarySlice<'a>>, u32, u64) {
        (self.table_polys, self.fri_domain_len, self.total_cols)
    }
}

impl Engine for CodewordEngine<'_> {
    type Output = (Mary, usize, MerkHeap);

    #[tracing::instrument(skip_all)]
    fn reduce_cpu(self) -> Self::Output {
        crate::codefuscate! {
            // ::
            // ::  this mary is a list of all tables' columns, extended to codewords
            // =/  codewords=mary
            //   (compute-lde table-polys fri-domain-len total-cols)
            let mut codewords = Mary {
                step: self.fri_domain_len,
                len: self.total_cols as u32,
                dat: vec![0; self.fri_domain_len as usize * self.total_cols as usize],
            };
            compute_lde::<Belt>(
                &self.table_polys,
                self.fri_domain_len,
                self.total_cols,
                codewords.as_mut_slice(),
            );
            // ::
            // ::  this mary is a list of rows, each row the values of above codewords at a fixed domain elt
            // =/  codeword-array=mary
            //   (transpose-bpolys codewords)
            let mut codeword_array = Mary {
                dat: vec![0; codewords.dat.len()],
                step: codewords.len,
                len: codewords.step,
            };
            mary_transpose(codewords.as_slice(), 1, &mut codeword_array.as_mut_slice());
            // =/  merk-heap=(pair @ merk-heap:merkle)
            //   (bp-build-merk-heap:merkle codeword-array)
            let (height, mh) = build_merk_heap_impl::<Belt>(codeword_array.as_slice()).unwrap();

            (codeword_array, height, mh)
        }
    }

    #[cfg(feature = "gpu")]
    #[tracing::instrument(skip_all)]
    fn reduce_gpu(self, gpu: super::gpu::GpuHandle) -> Self::Output {
        crate::codefuscate! {
            use nbx_tip5::melt::Melt;
            use nbx_tip5::tip5::DIGEST_LENGTH;

            use super::gpu::Submittable;
            use crate::utils::xeb;

            let height = xeb(self.fri_domain_len as usize);

            let res = Submittable::gpu_process(self, gpu);
            let codeword_array = res.codeword_array;

            let mh = MerkHeap {
                h: <[u64; DIGEST_LENGTH]>::try_from(&res.merk_heap.dat[..DIGEST_LENGTH])
                    .unwrap()
                    .map(Melt::from_u64),
                m: res.merk_heap,
            };

            (codeword_array, height, mh)
        }
    }
}
