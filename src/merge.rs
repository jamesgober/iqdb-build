//! Combining independently built sub-indexes into one.
//!
//! [`build_parallel`](IndexBuilder::build_parallel) produces a `Vec` of shards;
//! merging folds them back into a single index. The *mechanism* differs per
//! backend — a flat index appends rows, an IVF index extends its posting lists,
//! a graph index re-runs its boundary heuristics — so it cannot be expressed
//! through the read-only [`IndexCore`](iqdb_index::IndexCore) surface. The
//! [`Mergeable`] trait is the seam: a backend opts in by saying how to absorb
//! another instance of itself, and [`merge`] / [`IndexBuilder::build_merged`]
//! orchestrate the fold.

use iqdb_index::Index;
use iqdb_types::Result;

use crate::builder::{BuildItem, IndexBuilder};

/// A backend that can absorb another instance of itself.
///
/// Implement this for an [`Index`] so [`merge`] and
/// [`IndexBuilder::build_merged`] can combine sharded builds into one index. The
/// trait deliberately takes `other` **by value**: merging consumes the source,
/// letting an implementation move its storage rather than copy it.
///
/// # Contract
///
/// - **Same shape.** `other` must share `self`'s `dim` and `metric`. An
///   implementation should return
///   [`InvalidConfig`](iqdb_types::IqdbError::InvalidConfig) on a mismatch rather
///   than silently producing a corrupt index.
/// - **Set union.** After `self.merge(other)?`, every id searchable in either
///   input is searchable in `self`. This is where cross-shard id collisions
///   surface: if both inputs hold the same id, an implementation returns
///   [`Duplicate`](iqdb_types::IqdbError::Duplicate).
/// - **Failure is observable, not silent.** On `Err`, `self` may be left
///   partially merged; the operation is not transactional. Callers that need the
///   original back should merge into a clone.
///
/// # Examples
///
/// A flat index merges by appending the other's rows:
///
/// ```
/// use std::sync::Arc;
/// use iqdb_build::{Mergeable, merge};
/// use iqdb_types::{DistanceMetric, VectorId};
/// # use iqdb_index::{Index, IndexCore, IndexStats};
/// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
/// # #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
/// # #[derive(Clone, Default)] struct FlatConfig;
/// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
/// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
/// impl Mergeable for Flat {
///     fn merge(&mut self, other: Self) -> Result<()> {
///         if other.dim() != self.dim() || other.metric() != self.metric() {
///             return Err(IqdbError::InvalidConfig { reason: "merge shape mismatch" });
///         }
///         for (id, vector) in other.rows {
///             self.insert(id, vector, None)?; // re-checks duplicates
///         }
///         Ok(())
///     }
/// }
/// # fn main() -> iqdb_types::Result<()> {
/// let mut a = Flat::new(1, DistanceMetric::Euclidean, FlatConfig::default())?;
/// a.insert(VectorId::from(1u64), Arc::from([0.0_f32].as_slice()), None)?;
/// let mut b = Flat::new(1, DistanceMetric::Euclidean, FlatConfig::default())?;
/// b.insert(VectorId::from(2u64), Arc::from([1.0_f32].as_slice()), None)?;
///
/// a.merge(b)?;
/// assert_eq!(a.len(), 2);
/// # Ok(()) }
/// ```
pub trait Mergeable: Index {
    /// Absorb `other` into `self`, leaving `self` searchable over the union of
    /// both. See the [trait contract](Mergeable#contract) for the guarantees and
    /// failure modes.
    fn merge(&mut self, other: Self) -> Result<()>;
}

/// Fold a collection of sub-indexes into one by merging them in order.
///
/// Returns `Ok(None)` for an empty input, and otherwise merges every later index
/// into the first and returns it. This is the natural companion to
/// [`IndexBuilder::build_parallel`]: build shards, then `merge` them.
///
/// # Errors
///
/// Propagates the first [`Mergeable::merge`] error (for example
/// [`Duplicate`](iqdb_types::IqdbError::Duplicate) on a cross-shard id
/// collision). Because `merge` is not transactional, a failure partway leaves
/// the accumulator partially merged; the error is returned and the partial
/// accumulator dropped.
///
/// # Examples
///
/// ```
/// # use std::sync::Arc;
/// # use iqdb_build::{IndexBuilder, Mergeable, merge};
/// # use iqdb_types::{DistanceMetric, IqdbError, VectorId};
/// # use iqdb_index::{Index, IndexCore, IndexStats};
/// # use iqdb_types::{Hit, Metadata, Result, SearchParams};
/// # #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
/// # #[derive(Clone, Default)] struct FlatConfig;
/// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
/// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
/// # impl Mergeable for Flat { fn merge(&mut self, other: Self) -> Result<()> { for (id, v) in other.rows { self.insert(id, v, None)?; } Ok(()) } }
/// # fn main() -> iqdb_types::Result<()> {
/// let items: Vec<_> = (0u64..100).map(|i| (VectorId::from(i), Arc::from([i as f32].as_slice()), None)).collect();
/// let shards: Vec<Flat> = IndexBuilder::new(1, DistanceMetric::Euclidean).with_shards(4).build_parallel(items)?;
///
/// let one = merge(shards)?.expect("non-empty");
/// assert_eq!(one.len(), 100);
/// # Ok(()) }
/// ```
pub fn merge<I: Mergeable>(indexes: Vec<I>) -> Result<Option<I>> {
    let mut iter = indexes.into_iter();
    let Some(mut accumulator) = iter.next() else {
        return Ok(None);
    };
    for next in iter {
        accumulator.merge(next)?;
    }
    Ok(Some(accumulator))
}

impl<I: Mergeable> IndexBuilder<I> {
    /// Build the input in parallel and merge the shards into a single index.
    ///
    /// This is the full bulk pipeline — *split → build in parallel → merge* — in
    /// one call. It runs [`build_parallel`](IndexBuilder::build_parallel) (so the
    /// shard count, [`with_shards`](IndexBuilder::with_shards), and any
    /// [`on_progress`](IndexBuilder::on_progress) callback all apply), then folds
    /// the shards with [`merge`].
    ///
    /// # Errors
    ///
    /// Any error from the parallel build ([`InvalidConfig`](iqdb_types::IqdbError::InvalidConfig),
    /// [`DimensionMismatch`](iqdb_types::IqdbError::DimensionMismatch), within-shard
    /// [`Duplicate`](iqdb_types::IqdbError::Duplicate)) or from
    /// [`Mergeable::merge`] (notably a cross-shard `Duplicate`).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use iqdb_build::{IndexBuilder, Mergeable};
    /// use iqdb_types::{DistanceMetric, VectorId};
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
    /// # #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len()!=self.dim {return Err(IqdbError::DimensionMismatch{expected:self.dim,found:v.len()});} if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{ if dim==0 {return Err(IqdbError::InvalidConfig{reason:"dim must be > 0"});} Ok(Flat{dim,metric,rows:Vec::new()}) } }
    /// # impl Mergeable for Flat { fn merge(&mut self, other: Self) -> Result<()> { for (id, v) in other.rows { self.insert(id, v, None)?; } Ok(()) } }
    /// # fn main() -> iqdb_types::Result<()> {
    /// let items: Vec<_> = (0u64..1_000)
    ///     .map(|i| (VectorId::from(i), Arc::from([i as f32, 0.0].as_slice()), None))
    ///     .collect();
    ///
    /// // One call: shard, build across cores, merge back to a single index.
    /// let index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
    ///     .with_shards(8)
    ///     .build_merged(items)?;
    ///
    /// assert_eq!(index.len(), 1_000);
    /// # Ok(()) }
    /// ```
    pub fn build_merged<It>(&self, items: It) -> Result<I>
    where
        It: IntoIterator<Item = BuildItem>,
        I::Config: Send + Sync,
    {
        let shards = self.build_parallel(items)?;
        match merge(shards)? {
            Some(index) => Ok(index),
            // `build_parallel` always yields at least one shard, so this arm is
            // unreachable in practice; constructing an empty index keeps the
            // function total without an `unwrap`/`expect`.
            None => I::new(self.dim, self.metric, self.config.clone()),
        }
    }
}
