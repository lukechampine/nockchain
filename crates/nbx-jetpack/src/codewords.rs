use zkvm_jetpack::form::mary::{Mary, MarySlice};
use zkvm_jetpack::form::math::mary::mary_transpose;
use zkvm_jetpack::form::Belt;

use crate::eight::compute_lde;
use crate::three::{build_merk_heap_impl, MerkHeap};
use crate::utils::xeb;

#[cfg(feature = "gpu")]
use super::gpu;

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

    pub fn reduce(self) -> (Mary, usize, MerkHeap) {
        #[cfg(feature = "gpu")]
        if gpu::should_use_gpu() {
            self.reduce_gpu()
        } else {
            self.reduce_cpu()
        }

        #[cfg(not(feature = "gpu"))]
        self.reduce_cpu()
    }

    #[tracing::instrument(skip_all)]
    #[cfg(feature = "gpu")]
    pub fn reduce_gpu(self) -> (Mary, usize, MerkHeap) {
        use super::gpu::Submittable;

        //let height = xeb(self.fri_domain_len as usize);

        let res = Submittable::gpu_process(self);
        let codeword_array = res.codeword_array;

        /*let mut codeword_array = Mary {
            dat: vec![0; codewords.dat.len()],
            step: codewords.len,
            len: codewords.step,
        };
        mary_transpose(codewords.as_slice(), 1, &mut codeword_array.as_mut_slice());*/
        // =/  merk-heap=(pair @ merk-heap:merkle)
        //   (bp-build-merk-heap:merkle codeword-array)
        let (height, mh) = build_merk_heap_impl::<Belt>(codeword_array.as_slice()).unwrap();

        (codeword_array, height, mh)
    }

    #[tracing::instrument(skip_all)]
    pub fn reduce_cpu(self) -> (Mary, usize, MerkHeap) {
        // ::
        // ::  this mary is a list of all tables' columns, extended to codewords
        // =/  codewords=mary
        //   (compute-lde table-polys fri-domain-len total-cols)
        let mut codewords = Mary {
            step: self.fri_domain_len,
            len: self.total_cols as u32,
            dat: vec![0; self.fri_domain_len as usize * self.total_cols as usize],
        };
        compute_lde::<Belt>(&self.table_polys, self.fri_domain_len, self.total_cols, codewords.as_mut_slice());
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
