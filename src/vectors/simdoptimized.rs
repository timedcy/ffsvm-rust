use rand::{ChaChaRng, Rand, Rng};
use std::{
    fmt, iter::IntoIterator, marker::{Copy, Sized}, ops::{Index, IndexMut},
};

use random::Randomize;

/// Basic "matrix' we use for fast SIMD and parallel operations.
///
/// Note: Right now we use a Matrix mostly as a vector of vectors and is mostly
/// intended for read operations.
#[derive(Clone)]
pub struct SimdOptimized<T>
where
    T: Copy + Sized,
{
    /// Number of vectors this matrix has
    pub vectors: usize,

    /// Number of attributes this matrix has per subvector
    pub attributes: usize,

    /// Actual length of vectors
    pub vector_length: usize,

    /// We store all data in one giant array for performance reasons (caching)
    pub data: Vec<T>,
}

/// Basic iterator struct to go over matrix
pub struct IterManyVectors<'a, T: 'a>
where
    T: Copy + Sized,
{
    /// Reference to the matrix we iterate over.
    pub matrix: &'a SimdOptimized<T>,

    /// Current index of vector iteration.
    pub index: usize,
}

impl<T> SimdOptimized<T>
where
    T: Copy + Sized,
{
    /// Creates a new empty Matrix.
    pub fn with_dimension(vectors: usize, attributes: usize, default: T) -> SimdOptimized<T> {
        // TODO: Remove this. Was used to pad arrays to SIMD lenghts, but `faster` handles this now.
        let preferred_length = attributes; // util::prefered_simd_size(attributes);

        SimdOptimized::<T> {
            vectors,
            attributes,
            vector_length: preferred_length,
            data: vec![default; vectors * preferred_length],
        }
    }

    /// Sets a vector with the given data.
    pub fn set_vector(&mut self, index_vector: usize, vector: &[T]) {
        let start_index = self.offset(index_vector, 0);
        let src = &vector[.. self.attributes];

        self.data[start_index .. (self.attributes + start_index)].clone_from_slice(src);
    }

    /// Computes an offset for a vector and attribute.
    #[inline]
    pub fn offset(&self, vector: usize, attribute: usize) -> usize {
        (vector * self.vector_length + attribute)
    }
}

impl<T> Index<(usize, usize)> for SimdOptimized<T>
where
    T: Copy + Sized,
{
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &T {
        let index = self.offset(index.0, index.1);
        &self.data[index]
    }
}

impl<T> IndexMut<(usize, usize)> for SimdOptimized<T>
where
    T: Copy + Sized,
{
    fn index_mut(&mut self, index: (usize, usize)) -> &mut T {
        let index = self.offset(index.0, index.1);
        &mut self.data[index]
    }
}

impl<T> Index<usize> for SimdOptimized<T>
where
    T: Copy + Sized,
{
    type Output = [T];

    fn index(&self, index: usize) -> &[T] {
        let start_index = self.offset(index, 0);
        &self.data[start_index .. start_index + self.vector_length]
    }
}

impl<T> IndexMut<usize> for SimdOptimized<T>
where
    T: Copy + Sized,
{
    fn index_mut(&mut self, index: usize) -> &mut [T] {
        let start_index = self.offset(index, 0);
        &mut self.data[start_index .. start_index + self.vector_length]
    }
}

impl<T> fmt::Debug for SimdOptimized<T>
where
    T: Copy + Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(SimdOptimized {}, {}, [data])",
            self.vectors, self.attributes
        )
    }
}

impl<'a, T> IntoIterator for &'a SimdOptimized<T>
where
    T: Copy + Sized,
{
    type Item = &'a [T];
    type IntoIter = IterManyVectors<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        IterManyVectors {
            matrix: self,
            index: 0,
        }
    }
}

impl<'a, T> Iterator for IterManyVectors<'a, T>
where
    T: Copy + Sized,
{
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.matrix.vectors {
            None
        } else {
            self.index += 1;
            Some(&self.matrix[self.index - 1])
        }
    }
}

impl<T> Randomize for SimdOptimized<T>
where
    T: Sized + Copy + Rand,
{
    fn randomize(mut self) -> Self {
        let mut rng = ChaChaRng::new_unseeded();

        for i in 0 .. self.vectors {
            let gen = rng.gen_iter::<T>();
            let vector = gen.take(self.attributes).collect::<Vec<T>>();

            self.set_vector(i, vector.as_slice());
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use vectors::simdoptimized::SimdOptimized;

    #[test]
    fn test_iter() {
        let matrix = SimdOptimized::with_dimension(10, 5, 0);
        for x in &matrix {
            assert_eq!(x[0], 0);
        }
    }
}
