//! Data-parallel sharded construction.
//!
//! A single [`Index`] cannot be built concurrently — every
//! [`insert`](iqdb_index::IndexCore::insert) takes `&mut self`. The bulk path is
//! instead to **split the input into shards and build one sub-index per shard in
//! parallel**, then combine them. This module adds the parallel-build half;
//! merging the shards into one index lands in a later phase (see
//! [`dev/ROADMAP.md`](../dev/ROADMAP.md)). Until then the shards are directly
//! usable by the engine, whose storage is itself sharded.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use iqdb_index::Index;
use iqdb_types::Result;

use crate::builder::{BuildItem, IndexBuilder};

/// A progress snapshot delivered to the callback registered with
/// [`IndexBuilder::on_progress`] as parallel construction proceeds.
///
/// One snapshot is reported each time a shard finishes building, so
/// `shards_completed` climbs from `1` to `shards_total`. Because shards build
/// concurrently, snapshots may arrive on different threads and (rarely) slightly
/// out of order; treat `shards_completed` as a monotonic high-water count for a
/// progress bar, not as a strict sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildProgress {
    /// How many shards have finished building so far.
    pub shards_completed: usize,
    /// The total number of shards in this build.
    pub shards_total: usize,
}

impl<I: Index> IndexBuilder<I> {
    /// Set the shard count used by [`build_parallel`](IndexBuilder::build_parallel).
    ///
    /// The default is "auto" — one shard per available CPU
    /// ([`std::thread::available_parallelism`]). Set it explicitly to cap
    /// parallelism, to match the engine's shard layout, or for reproducible
    /// shard boundaries in tests. A count of `0` is treated as `1`, and the
    /// effective count never exceeds the number of items (no empty shards).
    ///
    /// This is a consuming, immutable setter: it returns a new builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::DistanceMetric;
    /// # use std::sync::Arc;
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, Metadata, Result, SearchParams, VectorId};
    /// # struct Flat { dim: usize, metric: DistanceMetric }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self,_i:VectorId,_v:Arc<[f32]>,_m:Option<Metadata>)->Result<()>{Ok(())} fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{0} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric})} }
    /// let builder = IndexBuilder::<Flat>::new(8, DistanceMetric::Euclidean).with_shards(4);
    /// assert_eq!(builder.shards(), Some(4));
    /// ```
    #[inline]
    #[must_use]
    pub fn with_shards(mut self, shards: usize) -> Self {
        self.shards = Some(shards);
        self
    }

    /// The configured shard count, or `None` when it is left to "auto".
    ///
    /// # Examples
    ///
    /// ```
    /// # use iqdb_build::IndexBuilder;
    /// # use iqdb_types::DistanceMetric;
    /// # use std::sync::Arc;
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, Metadata, Result, SearchParams, VectorId};
    /// # struct Flat { dim: usize, metric: DistanceMetric }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self,_i:VectorId,_v:Arc<[f32]>,_m:Option<Metadata>)->Result<()>{Ok(())} fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{0} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric})} }
    /// let builder = IndexBuilder::<Flat>::new(8, DistanceMetric::Euclidean);
    /// assert_eq!(builder.shards(), None);
    /// ```
    #[inline]
    pub fn shards(&self) -> Option<usize> {
        self.shards
    }

    /// Register a callback invoked each time a shard finishes building during
    /// [`build_parallel`](IndexBuilder::build_parallel) (and the build phase of
    /// [`build_merged`](IndexBuilder::build_merged)).
    ///
    /// The callback receives a [`BuildProgress`] snapshot. It must be
    /// `Send + Sync` because shards build concurrently, so it may be called from
    /// several worker threads; keep it cheap and non-blocking (update an atomic,
    /// nudge a progress bar). The sequential [`build`](IndexBuilder::build) does
    /// not report progress.
    ///
    /// This is a consuming, immutable setter: it returns a new builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::{DistanceMetric, VectorId};
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, Metadata, Result, SearchParams};
    /// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
    /// # fn main() -> iqdb_types::Result<()> {
    /// let calls = Arc::new(AtomicUsize::new(0));
    /// let calls2 = Arc::clone(&calls);
    ///
    /// let items: Vec<_> = (0u64..100)
    ///     .map(|i| (VectorId::from(i), Arc::from([i as f32].as_slice()), None))
    ///     .collect();
    ///
    /// let _shards: Vec<Flat> = IndexBuilder::new(1, DistanceMetric::Euclidean)
    ///     .with_shards(4)
    ///     .on_progress(move |p| {
    ///         assert_eq!(p.shards_total, 4);
    ///         calls2.fetch_add(1, Ordering::Relaxed);
    ///     })
    ///     .build_parallel(items)?;
    ///
    /// // The callback fired once per shard.
    /// assert_eq!(calls.load(Ordering::Relaxed), 4);
    /// # Ok(()) }
    /// ```
    #[inline]
    #[must_use]
    pub fn on_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(BuildProgress) + Send + Sync + 'static,
    {
        self.progress = Some(Arc::new(callback));
        self
    }

    /// Build the input as several independent sub-indexes, one per shard, in
    /// parallel.
    ///
    /// The items are split into contiguous, near-equal shards and each shard is
    /// constructed on rayon's work-stealing pool: every shard runs
    /// [`Index::new`] with this builder's `dim`/`metric`/`config` and inserts its
    /// slice through [`insert_batch`](iqdb_index::IndexCore::insert_batch). The
    /// returned `Vec` preserves shard order (shard 0 holds the first slice of
    /// input, and so on).
    ///
    /// Wall-clock build time drops roughly linearly with cores, because the
    /// shards share nothing. Combining them into a single index is the job of a
    /// later merge phase; meanwhile the shards are directly usable by the engine,
    /// whose storage is sharded.
    ///
    /// The shard count comes from [`with_shards`](IndexBuilder::with_shards), or
    /// is one-per-CPU when left to auto. It is capped at the item count, so you
    /// never get an empty shard (except the single empty index produced from
    /// empty input).
    ///
    /// # Errors
    ///
    /// The same construction and insertion errors as [`build`](IndexBuilder::build),
    /// surfaced from whichever shard hits them first:
    /// [`InvalidConfig`](iqdb_types::IqdbError::InvalidConfig),
    /// [`DimensionMismatch`](iqdb_types::IqdbError::DimensionMismatch), and
    /// [`Duplicate`](iqdb_types::IqdbError::Duplicate).
    ///
    /// Note that duplicate detection is **per shard**: two items with the same id
    /// in *different* shards are not caught here (they would collide at merge
    /// time). Within a shard, a repeated id errors as usual.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::{DistanceMetric, VectorId};
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
    /// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len()!=self.dim { return Err(IqdbError::DimensionMismatch{expected:self.dim,found:v.len()});} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{ if dim==0 {return Err(IqdbError::InvalidConfig{reason:"dim must be > 0"});} Ok(Flat{dim,metric,rows:Vec::new()}) } }
    /// # fn main() -> iqdb_types::Result<()> {
    /// let items: Vec<_> = (0u64..1_000)
    ///     .map(|i| (VectorId::from(i), Arc::from([i as f32, 0.0].as_slice()), None))
    ///     .collect();
    ///
    /// // Build four shards in parallel.
    /// let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
    ///     .with_shards(4)
    ///     .build_parallel(items)?;
    ///
    /// assert_eq!(shards.len(), 4);
    /// let total: usize = shards.iter().map(|s| s.len()).sum();
    /// assert_eq!(total, 1_000);
    /// # Ok(()) }
    /// ```
    pub fn build_parallel<It>(&self, items: It) -> Result<Vec<I>>
    where
        It: IntoIterator<Item = BuildItem>,
        I::Config: Send + Sync,
    {
        let all: Vec<BuildItem> = items.into_iter().collect();
        let shard_count = self.resolve_shards(all.len());
        let chunks = split_items(all, shard_count);
        let total = chunks.len();
        let completed = AtomicUsize::new(0);

        chunks
            .into_par_iter()
            .map(|chunk| {
                let mut index = I::new(self.dim(), self.metric(), self.config().clone())?;
                index.insert_batch(chunk)?;
                if let Some(callback) = self.progress.as_deref() {
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    callback(BuildProgress {
                        shards_completed: done,
                        shards_total: total,
                    });
                }
                Ok(index)
            })
            .collect()
    }

    /// Resolve the effective shard count: requested-or-auto, clamped to
    /// `1..=len` (never zero, never more shards than items).
    fn resolve_shards(&self, len: usize) -> usize {
        let requested = match self.shards() {
            Some(n) => n.max(1),
            None => std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        };
        requested.min(len.max(1))
    }
}

/// Split `all` into `k` contiguous, near-equal owned chunks. The first
/// `len % k` chunks get one extra item, so sizes differ by at most one.
fn split_items(all: Vec<BuildItem>, k: usize) -> Vec<Vec<BuildItem>> {
    let len = all.len();
    let base = len / k;
    let remainder = len % k;
    let mut iter = all.into_iter();
    let mut chunks = Vec::with_capacity(k);
    for i in 0..k {
        let take = base + usize::from(i < remainder);
        chunks.push(iter.by_ref().take(take).collect());
    }
    chunks
}
