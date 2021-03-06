mod linear;
mod poly;
mod rbf;
mod sigmoid;

use crate::sparse::{SparseMatrix, SparseVector};
use simd_aligned::{f32s, RowOptimized, SimdMatrix, SimdVector};

pub use self::{linear::*, poly::*, rbf::*, sigmoid::*};

/// Base trait for kernels
#[doc(hidden)]
pub trait KernelDense
where
    Self: Sync,
{
    fn compute(&self, vectors: &SimdMatrix<f32s, RowOptimized>, feature: &SimdVector<f32s>, output: &mut [f64]);
}

/// Base trait for kernels
#[doc(hidden)]
pub trait KernelSparse
where
    Self: Sync,
{
    fn compute(&self, vectors: &SparseMatrix<f32>, feature: &SparseVector<f32>, output: &mut [f64]);
}
