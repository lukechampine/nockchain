#[cfg(feature = "gpu")]
pub use rayon::prelude;

#[cfg(not(feature = "gpu"))]
pub mod prelude {
    use std::cmp::Ordering;
    use std::iter as si;

    #[repr(transparent)]
    pub struct SeqIterator<I>
    where
        I: Iterator,
    {
        iter: I,
    }

    impl<I> SeqIterator<I>
    where
        I: Iterator,
    {
        #[inline(always)]
        pub fn new<T>(iterable: T) -> Self
        where
            T: IntoIterator<IntoIter = I>,
        {
            Self {
                iter: iterable.into_iter(),
            }
        }

        #[inline(always)]
        pub fn from_iter(iter: I) -> Self {
            Self { iter }
        }

        // Consuming operations
        #[inline(always)]
        pub fn for_each<F>(self, f: F)
        where
            F: FnMut(I::Item),
        {
            self.iter.for_each(f)
        }

        #[inline(always)]
        pub fn count(self) -> usize {
            self.iter.count()
        }

        #[inline(always)]
        pub fn reduce<OP, ID>(self, identity: ID, op: OP) -> I::Item
        where
            OP: Fn(I::Item, I::Item) -> I::Item + Sync + Send,
            ID: Fn() -> I::Item + Sync + Send,
        {
            self.iter.fold(identity(), op)
        }

        #[inline(always)]
        pub fn reduce_with<OP>(self, op: OP) -> Option<I::Item>
        where
            OP: Fn(I::Item, I::Item) -> I::Item + Sync + Send,
        {
            self.iter.reduce(op)
        }

        #[inline(always)]
        pub fn fold<T, ID, F>(self, identity: ID, fold_op: F) -> SeqIterator<si::Once<T>>
        where
            F: Fn(T, I::Item) -> T + Sync + Send,
            ID: Fn() -> T + Sync + Send,
            T: Send,
        {
            // Wrap the result into an iterator to match Rayon's API
            let result = self.iter.fold(identity(), fold_op);
            SeqIterator::from_iter(si::once(result))
        }

        #[inline(always)]
        pub fn fold_with<F, T>(self, init: T, fold_op: F) -> SeqIterator<si::Once<T>>
        where
            F: Fn(T, I::Item) -> T + Sync + Send,
            T: Send + Clone,
        {
            let result = self.iter.fold(init, fold_op);
            SeqIterator::from_iter(si::once(result))
        }

        #[inline(always)]
        pub fn sum<S>(self) -> S
        where
            S: si::Sum<I::Item>,
        {
            self.iter.sum()
        }

        #[inline(always)]
        pub fn product<P>(self) -> P
        where
            P: si::Product<I::Item>,
        {
            self.iter.product()
        }

        #[inline(always)]
        pub fn min(self) -> Option<I::Item>
        where
            I::Item: Ord,
        {
            self.iter.min()
        }

        #[inline(always)]
        pub fn min_by<F>(self, compare: F) -> Option<I::Item>
        where
            F: FnMut(&I::Item, &I::Item) -> Ordering,
        {
            self.iter.min_by(compare)
        }

        #[inline(always)]
        pub fn min_by_key<K, F>(self, f: F) -> Option<I::Item>
        where
            K: Ord,
            F: FnMut(&I::Item) -> K,
        {
            self.iter.min_by_key(f)
        }

        #[inline(always)]
        pub fn max(self) -> Option<I::Item>
        where
            I::Item: Ord,
        {
            self.iter.max()
        }

        #[inline(always)]
        pub fn max_by<F>(self, compare: F) -> Option<I::Item>
        where
            F: FnMut(&I::Item, &I::Item) -> Ordering,
        {
            self.iter.max_by(compare)
        }

        #[inline(always)]
        pub fn max_by_key<K, F>(self, f: F) -> Option<I::Item>
        where
            K: Ord,
            F: FnMut(&I::Item) -> K,
        {
            self.iter.max_by_key(f)
        }

        #[inline(always)]
        pub fn find<P>(mut self, predicate: P) -> Option<I::Item>
        where
            P: FnMut(&I::Item) -> bool,
        {
            self.iter.find(predicate)
        }

        #[inline(always)]
        pub fn find_any<P>(mut self, predicate: P) -> Option<I::Item>
        where
            P: Fn(&I::Item) -> bool + Sync + Send,
        {
            self.iter.find(predicate)
        }

        #[inline(always)]
        pub fn find_first<P>(mut self, predicate: P) -> Option<I::Item>
        where
            P: Fn(&I::Item) -> bool + Sync + Send,
        {
            self.iter.find(predicate)
        }

        #[inline(always)]
        pub fn any<P>(mut self, predicate: P) -> bool
        where
            P: FnMut(I::Item) -> bool,
        {
            self.iter.any(predicate)
        }

        #[inline(always)]
        pub fn all<P>(mut self, predicate: P) -> bool
        where
            P: FnMut(I::Item) -> bool,
        {
            self.iter.all(predicate)
        }

        #[inline(always)]
        pub fn collect<C>(self) -> C
        where
            C: si::FromIterator<I::Item>,
        {
            self.iter.collect()
        }

        #[inline(always)]
        pub fn partition<B, F>(self, f: F) -> (B, B)
        where
            B: Default + Extend<I::Item>,
            F: FnMut(&I::Item) -> bool,
        {
            self.iter.partition(f)
        }

        // Adapter operations (return new SeqIterator)
        #[inline(always)]
        pub fn map<F, B>(self, f: F) -> SeqIterator<si::Map<I, F>>
        where
            F: FnMut(I::Item) -> B,
        {
            SeqIterator {
                iter: self.iter.map(f),
            }
        }

        #[inline(always)]
        pub fn filter<P>(self, predicate: P) -> SeqIterator<si::Filter<I, P>>
        where
            P: FnMut(&I::Item) -> bool,
        {
            SeqIterator {
                iter: self.iter.filter(predicate),
            }
        }

        #[inline(always)]
        pub fn filter_map<F, B>(self, f: F) -> SeqIterator<si::FilterMap<I, F>>
        where
            F: FnMut(I::Item) -> Option<B>,
        {
            SeqIterator {
                iter: self.iter.filter_map(f),
            }
        }

        #[inline(always)]
        pub fn enumerate(self) -> SeqIterator<si::Enumerate<I>> {
            SeqIterator {
                iter: self.iter.enumerate(),
            }
        }

        #[inline(always)]
        pub fn skip(self, n: usize) -> SeqIterator<si::Skip<I>> {
            SeqIterator {
                iter: self.iter.skip(n),
            }
        }

        #[inline(always)]
        pub fn take(self, n: usize) -> SeqIterator<si::Take<I>> {
            SeqIterator {
                iter: self.iter.take(n),
            }
        }

        #[inline(always)]
        pub fn step_by(self, step: usize) -> SeqIterator<si::StepBy<I>> {
            SeqIterator {
                iter: self.iter.step_by(step),
            }
        }

        #[inline(always)]
        pub fn chain<U>(self, other: U) -> SeqIterator<si::Chain<I, U::IntoIter>>
        where
            U: IntoIterator<Item = I::Item>,
        {
            SeqIterator {
                iter: self.iter.chain(other),
            }
        }

        #[inline(always)]
        pub fn zip<U>(self, other: U) -> SeqIterator<si::Zip<I, U::IntoIter>>
        where
            U: IntoIterator,
        {
            SeqIterator {
                iter: self.iter.zip(other),
            }
        }

        #[inline(always)]
        pub fn flat_map<U, F>(self, f: F) -> SeqIterator<si::FlatMap<I, U, F>>
        where
            U: IntoIterator,
            F: FnMut(I::Item) -> U,
        {
            SeqIterator {
                iter: self.iter.flat_map(f),
            }
        }

        #[inline(always)]
        pub fn flatten(self) -> SeqIterator<si::Flatten<I>>
        where
            I::Item: IntoIterator,
        {
            SeqIterator {
                iter: self.iter.flatten(),
            }
        }
    }

    impl<I> Iterator for SeqIterator<I>
    where
        I: Iterator,
    {
        type Item = I::Item;

        #[inline(always)]
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next()
        }

        #[inline(always)]
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.iter.size_hint()
        }

        #[inline(always)]
        fn count(self) -> usize {
            self.iter.count()
        }

        #[inline(always)]
        fn fold<B, F>(self, init: B, f: F) -> B
        where
            F: FnMut(B, Self::Item) -> B,
        {
            self.iter.fold(init, f)
        }
    }

    /// Trait for converting collections into our sequential iterator
    pub trait IntoParIterator {
        type Item;
        type Iter: Iterator<Item = Self::Item>;

        fn into_par_iter(self) -> SeqIterator<Self::Iter>;
    }

    /// Extension trait for slice chunk operations
    pub trait ParallelSlice<T> {
        /// Creates an iterator over chunks of the slice
        fn par_chunks(&self, chunk_size: usize) -> SeqIterator<std::slice::Chunks<'_, T>>;

        /// Creates an iterator over chunks of the slice of exactly `chunk_size` length
        fn par_chunks_exact(&self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExact<'_, T>>;
    }

    /// Extension trait for mutable slice chunk operations
    pub trait ParallelSliceMut<T> {
        /// Creates an iterator over mutable chunks of the slice
        fn par_chunks_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksMut<'_, T>>;

        /// Creates an iterator over mutable chunks of the slice of exactly `chunk_size` length
        fn par_chunks_exact_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExactMut<'_, T>>;
    }

    // Implement ParallelSlice for slices
    impl<T> ParallelSlice<T> for [T] {
        #[inline(always)]
        fn par_chunks(&self, chunk_size: usize) -> SeqIterator<std::slice::Chunks<'_, T>> {
            SeqIterator::from_iter(self.chunks(chunk_size))
        }

        #[inline(always)]
        fn par_chunks_exact(&self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExact<'_, T>> {
            SeqIterator::from_iter(self.chunks_exact(chunk_size))
        }
    }

    // Implement ParallelSliceMut for mutable slices
    impl<T> ParallelSliceMut<T> for [T] {
        #[inline(always)]
        fn par_chunks_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksMut<'_, T>> {
            SeqIterator::from_iter(self.chunks_mut(chunk_size))
        }

        #[inline(always)]
        fn par_chunks_exact_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExactMut<'_, T>> {
            SeqIterator::from_iter(self.chunks_exact_mut(chunk_size))
        }
    }

    // Convenience implementations for Vec
    impl<T> ParallelSlice<T> for Vec<T> {
        #[inline(always)]
        fn par_chunks(&self, chunk_size: usize) -> SeqIterator<std::slice::Chunks<'_, T>> {
            self.as_slice().par_chunks(chunk_size)
        }

        #[inline(always)]
        fn par_chunks_exact(&self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExact<'_, T>> {
            self.as_slice().par_chunks_exact(chunk_size)
        }
    }

    impl<T> ParallelSliceMut<T> for Vec<T> {
        #[inline(always)]
        fn par_chunks_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksMut<'_, T>> {
            self.as_mut_slice().par_chunks_mut(chunk_size)
        }

        #[inline(always)]
        fn par_chunks_exact_mut(&mut self, chunk_size: usize) -> SeqIterator<std::slice::ChunksExactMut<'_, T>> {
            self.as_mut_slice().par_chunks_exact_mut(chunk_size)
        }
    }

    // Implement for Vec
    impl<T> IntoParIterator for Vec<T> {
        type Item = T;
        type Iter = std::vec::IntoIter<T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self.into_iter())
        }
    }

    // Implement for Vec references
    impl<'a, T> IntoParIterator for &'a Vec<T> {
        type Item = &'a T;
        type Iter = std::slice::Iter<'a, T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self.iter())
        }
    }

    // Implement for slices
    impl<'a, T> IntoParIterator for &'a [T] {
        type Item = &'a T;
        type Iter = std::slice::Iter<'a, T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self.iter())
        }
    }

    // Implement for arrays
    impl<'a, T, const N: usize> IntoParIterator for &'a [T; N] {
        type Item = &'a T;
        type Iter = std::slice::Iter<'a, T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self.iter())
        }
    }

    // Implement for owned arrays
    impl<T, const N: usize> IntoParIterator for [T; N] {
        type Item = T;
        type Iter = std::array::IntoIter<T, N>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self.into_iter())
        }
    }

    // Implement for Range
    impl<T> IntoParIterator for std::ops::Range<T>
    where
        std::ops::Range<T>: Iterator<Item = T>,
    {
        type Item = T;
        type Iter = std::ops::Range<T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self)
        }
    }

    // Implement for RangeInclusive
    impl<T> IntoParIterator for std::ops::RangeInclusive<T>
    where
        std::ops::RangeInclusive<T>: Iterator<Item = T>,
    {
        type Item = T;
        type Iter = std::ops::RangeInclusive<T>;

        #[inline(always)]
        fn into_par_iter(self) -> SeqIterator<Self::Iter> {
            SeqIterator::from_iter(self)
        }
    }
}