//! The sequential builder and its Tier-1 free-function shortcuts.
//!
//! [`IndexBuilder`] is the configured (Tier-2) entry point; [`build`] and
//! [`build_into`] are the one-call (Tier-1) shortcuts over it.

use std::sync::Arc;

use iqdb_index::{Index, IndexCore};
use iqdb_types::{DistanceMetric, Metadata, Result, VectorId};

/// One vector to feed into a build: its [`VectorId`], its components owned as an
/// [`Arc<[f32]>`](std::sync::Arc), and its optional [`Metadata`].
///
/// This is exactly the element type of
/// [`IndexCore::insert_batch`](iqdb_index::IndexCore::insert_batch), so building
/// never reshapes or re-copies your data: the [`Arc`] you hand in is the
/// allocation the index stores. Vectors without metadata pass [`None`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use iqdb_build::BuildItem;
/// use iqdb_types::VectorId;
///
/// let item: BuildItem = (VectorId::from(7u64), Arc::from([0.1_f32, 0.2].as_slice()), None);
/// assert_eq!(item.0, VectorId::from(7u64));
/// assert_eq!(item.1.len(), 2);
/// ```
pub type BuildItem = (VectorId, Arc<[f32]>, Option<Metadata>);

/// A configured, reusable plan for constructing an [`Index`].
///
/// `IndexBuilder` holds the three things [`Index::new`] needs — the
/// dimensionality, the [`DistanceMetric`], and the backend's own
/// [`Config`](iqdb_index::Index::Config) — and turns a stream of [`BuildItem`]s
/// into a finished index with [`build`](IndexBuilder::build). It is the Tier-2
/// surface: reach for it when you want to tune the backend (graph degree, probe
/// count, …). For the common case with default configuration, the free function
/// [`build`](crate::build) is one call.
///
/// The same builder constructs **any** backend that implements [`Index`] — flat,
/// HNSW, IVF, or your own — because it never names a concrete type.
///
/// A builder is cheap to clone and reuse: the same plan can build many indexes
/// from different inputs.
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
/// # impl IndexCore for Flat {
/// #   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> {
/// #     if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); }
/// #     if self.rows.iter().any(|(e, _)| e == &id) { return Err(IqdbError::Duplicate); }
/// #     self.rows.push((id, v)); Ok(())
/// #   }
/// #   fn delete(&mut self, id: &VectorId) -> Result<()> { match self.rows.iter().position(|(e, _)| e == id) { Some(p) => { let _ = self.rows.remove(p); Ok(()) } None => Err(IqdbError::NotFound) } }
/// #   fn search(&self, q: &[f32], p: &SearchParams) -> Result<Vec<Hit>> { let mut h: Vec<Hit> = self.rows.iter().map(|(id, v)| Hit { id: id.clone(), distance: q.iter().zip(v.iter()).map(|(a, b)| (a - b) * (a - b)).sum(), metadata: None }).collect(); h.sort_by(|a, b| a.distance.total_cmp(&b.distance)); h.truncate(p.k); Ok(h) }
/// #   fn len(&self) -> usize { self.rows.len() }
/// #   fn dim(&self) -> usize { self.dim }
/// #   fn metric(&self) -> DistanceMetric { self.metric }
/// #   fn flush(&mut self) -> Result<()> { Ok(()) }
/// #   fn stats(&self) -> IndexStats { IndexStats { n_vectors: self.rows.len(), index_type: "flat", ..IndexStats::default() } }
/// # }
/// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { if dim == 0 { return Err(IqdbError::InvalidConfig { reason: "dim must be > 0" }); } Ok(Flat { dim, metric, rows: Vec::new() }) } }
/// # fn main() -> iqdb_types::Result<()> {
/// let builder = IndexBuilder::new(2, DistanceMetric::Euclidean);
/// let index: Flat = builder.build(vec![
///     (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
///     (VectorId::from(2u64), Arc::from([1.0_f32, 1.0].as_slice()), None),
/// ])?;
/// assert_eq!(index.len(), 2);
/// # Ok(()) }
/// ```
pub struct IndexBuilder<I: Index> {
    pub(crate) dim: usize,
    pub(crate) metric: DistanceMetric,
    pub(crate) config: I::Config,
    /// Shard count for [`build_parallel`](IndexBuilder::build_parallel). `None`
    /// means "auto" — one shard per available CPU. Ignored by the sequential
    /// [`build`](IndexBuilder::build).
    pub(crate) shards: Option<usize>,
    /// Optional progress callback, invoked as shards finish building. Wrapped in
    /// an [`Arc`] so the builder stays [`Clone`] and the closure can be shared
    /// across rayon worker threads.
    pub(crate) progress: Option<Arc<dyn Fn(crate::BuildProgress) + Send + Sync>>,
}

impl<I: Index> Clone for IndexBuilder<I> {
    fn clone(&self) -> Self {
        Self {
            dim: self.dim,
            metric: self.metric,
            config: self.config.clone(),
            shards: self.shards,
            progress: self.progress.clone(),
        }
    }
}

impl<I: Index> IndexBuilder<I> {
    /// Create a builder for a `dim`-dimensional index under `metric`, using the
    /// backend's default [`Config`](iqdb_index::Index::Config).
    ///
    /// Use [`with_config`](IndexBuilder::with_config) when the backend has
    /// parameters to tune.
    ///
    /// # Examples
    ///
    /// ```
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::DistanceMetric;
    /// # use std::sync::Arc;
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams, VectorId};
    /// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id, v)); Ok(()) } fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) } fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) } fn len(&self) -> usize { self.rows.len() } fn dim(&self) -> usize { self.dim } fn metric(&self) -> DistanceMetric { self.metric } fn flush(&mut self) -> Result<()> { Ok(()) } fn stats(&self) -> IndexStats { IndexStats::default() } }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { Ok(Flat { dim, metric, rows: Vec::new() }) } }
    /// let builder = IndexBuilder::<Flat>::new(128, DistanceMetric::Cosine);
    /// assert_eq!(builder.dim(), 128);
    /// assert_eq!(builder.metric(), DistanceMetric::Cosine);
    /// ```
    #[inline]
    pub fn new(dim: usize, metric: DistanceMetric) -> Self {
        Self {
            dim,
            metric,
            config: I::Config::default(),
            shards: None,
            progress: None,
        }
    }

    /// Create a builder with an explicit backend
    /// [`Config`](iqdb_index::Index::Config).
    ///
    /// This is the Tier-2 tuning entry point: pass the backend's own parameter
    /// struct (graph degree, cluster/probe counts, …) and every index this
    /// builder produces is constructed with it.
    ///
    /// # Examples
    ///
    /// ```
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::DistanceMetric;
    /// # use std::sync::Arc;
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams, VectorId};
    /// # struct Graph { dim: usize, metric: DistanceMetric, degree: usize, n: usize }
    /// # #[derive(Clone)] struct GraphConfig { degree: usize }
    /// # impl Default for GraphConfig { fn default() -> Self { Self { degree: 16 } } }
    /// # impl IndexCore for Graph { fn insert(&mut self, _id: VectorId, _v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.n += 1; Ok(()) } fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) } fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) } fn len(&self) -> usize { self.n } fn dim(&self) -> usize { self.dim } fn metric(&self) -> DistanceMetric { self.metric } fn flush(&mut self) -> Result<()> { Ok(()) } fn stats(&self) -> IndexStats { IndexStats::default() } }
    /// # impl Index for Graph { type Config = GraphConfig; fn new(dim: usize, metric: DistanceMetric, c: Self::Config) -> Result<Self> { Ok(Graph { dim, metric, degree: c.degree, n: 0 }) } }
    /// // Tune the graph degree away from its default of 16.
    /// let builder = IndexBuilder::<Graph>::with_config(64, DistanceMetric::Euclidean, GraphConfig { degree: 32 });
    /// assert_eq!(builder.config().degree, 32);
    /// ```
    #[inline]
    pub fn with_config(dim: usize, metric: DistanceMetric, config: I::Config) -> Self {
        Self {
            dim,
            metric,
            config,
            shards: None,
            progress: None,
        }
    }

    /// The dimensionality this builder constructs indexes for.
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
    /// # impl IndexCore for Flat { fn insert(&mut self, _id: VectorId, _v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { Ok(()) } fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) } fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) } fn len(&self) -> usize { 0 } fn dim(&self) -> usize { self.dim } fn metric(&self) -> DistanceMetric { self.metric } fn flush(&mut self) -> Result<()> { Ok(()) } fn stats(&self) -> IndexStats { IndexStats::default() } }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { Ok(Flat { dim, metric }) } }
    /// let builder = IndexBuilder::<Flat>::new(96, DistanceMetric::Cosine);
    /// assert_eq!(builder.dim(), 96);
    /// ```
    #[inline]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// The [`DistanceMetric`] this builder constructs indexes under.
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
    /// # impl IndexCore for Flat { fn insert(&mut self, _id: VectorId, _v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { Ok(()) } fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) } fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) } fn len(&self) -> usize { 0 } fn dim(&self) -> usize { self.dim } fn metric(&self) -> DistanceMetric { self.metric } fn flush(&mut self) -> Result<()> { Ok(()) } fn stats(&self) -> IndexStats { IndexStats::default() } }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { Ok(Flat { dim, metric }) } }
    /// let builder = IndexBuilder::<Flat>::new(96, DistanceMetric::Manhattan);
    /// assert_eq!(builder.metric(), DistanceMetric::Manhattan);
    /// ```
    #[inline]
    pub fn metric(&self) -> DistanceMetric {
        self.metric
    }

    /// The backend [`Config`](iqdb_index::Index::Config) this builder constructs
    /// indexes with.
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
    /// # #[derive(Clone, Default, Debug, PartialEq)] struct FlatConfig;
    /// # impl IndexCore for Flat { fn insert(&mut self, _id: VectorId, _v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { Ok(()) } fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) } fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) } fn len(&self) -> usize { 0 } fn dim(&self) -> usize { self.dim } fn metric(&self) -> DistanceMetric { self.metric } fn flush(&mut self) -> Result<()> { Ok(()) } fn stats(&self) -> IndexStats { IndexStats::default() } }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { Ok(Flat { dim, metric }) } }
    /// let builder = IndexBuilder::<Flat>::new(8, DistanceMetric::Euclidean);
    /// assert_eq!(builder.config(), &FlatConfig);
    /// ```
    #[inline]
    pub fn config(&self) -> &I::Config {
        &self.config
    }

    /// Construct a fresh index and bulk-insert every item into it.
    ///
    /// The index is created with this builder's `dim`, `metric`, and `config`
    /// via [`Index::new`], then every item is inserted through the backend's
    /// [`insert_batch`](iqdb_index::IndexCore::insert_batch) path (which a
    /// backend may accelerate). On success the returned index holds exactly the
    /// supplied vectors.
    ///
    /// # Errors
    ///
    /// - [`IqdbError::InvalidConfig`](iqdb_types::IqdbError::InvalidConfig) if
    ///   [`Index::new`] rejects the `dim`/`config` (for example `dim == 0`).
    /// - [`IqdbError::DimensionMismatch`](iqdb_types::IqdbError::DimensionMismatch)
    ///   if any item's vector length differs from `dim`.
    /// - [`IqdbError::Duplicate`](iqdb_types::IqdbError::Duplicate) if two items
    ///   share a [`VectorId`] (or one collides with an id the backend rejects).
    ///
    /// Insertion is fail-fast: the first failing item returns its error and the
    /// half-built index is dropped, so a failed `build` leaves nothing behind.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use iqdb_build::IndexBuilder;
    /// use iqdb_types::{DistanceMetric, IqdbError, VectorId};
    /// # use iqdb_index::{Index, IndexCore, IndexStats};
    /// # use iqdb_types::{Hit, Metadata, Result, SearchParams};
    /// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
    /// # #[derive(Clone, Default)] struct FlatConfig;
    /// # impl IndexCore for Flat {
    /// #   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); } if self.rows.iter().any(|(e, _)| e == &id) { return Err(IqdbError::Duplicate); } self.rows.push((id, v)); Ok(()) }
    /// #   fn delete(&mut self, id: &VectorId) -> Result<()> { match self.rows.iter().position(|(e, _)| e == id) { Some(p) => { let _ = self.rows.remove(p); Ok(()) } None => Err(IqdbError::NotFound) } }
    /// #   fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) }
    /// #   fn len(&self) -> usize { self.rows.len() }
    /// #   fn dim(&self) -> usize { self.dim }
    /// #   fn metric(&self) -> DistanceMetric { self.metric }
    /// #   fn flush(&mut self) -> Result<()> { Ok(()) }
    /// #   fn stats(&self) -> IndexStats { IndexStats { n_vectors: self.rows.len(), index_type: "flat", ..IndexStats::default() } }
    /// # }
    /// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { if dim == 0 { return Err(IqdbError::InvalidConfig { reason: "dim must be > 0" }); } Ok(Flat { dim, metric, rows: Vec::new() }) } }
    /// # fn main() -> iqdb_types::Result<()> {
    /// let index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean).build(vec![
    ///     (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
    ///     (VectorId::from(2u64), Arc::from([1.0_f32, 0.0].as_slice()), None),
    /// ])?;
    /// assert_eq!(index.len(), 2);
    ///
    /// // A duplicate id is rejected.
    /// let dup: Result<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean).build(vec![
    ///     (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
    ///     (VectorId::from(1u64), Arc::from([1.0_f32, 0.0].as_slice()), None),
    /// ]);
    /// assert!(matches!(dup, Err(IqdbError::Duplicate)));
    /// # Ok(()) }
    /// ```
    pub fn build<It>(&self, items: It) -> Result<I>
    where
        It: IntoIterator<Item = BuildItem>,
    {
        let mut index = I::new(self.dim, self.metric, self.config.clone())?;
        let _ = build_into(&mut index, items)?;
        Ok(index)
    }
}

/// Construct a fresh index from a stream of vectors, using the backend's default
/// [`Config`](iqdb_index::Index::Config).
///
/// This is the Tier-1 lazy path: the whole common case in one call. It is
/// shorthand for `IndexBuilder::new(dim, metric).build(items)`. Reach for
/// [`IndexBuilder::with_config`] instead when the backend needs tuning.
///
/// # Errors
///
/// Propagates the same errors as [`IndexBuilder::build`]:
/// [`IqdbError::InvalidConfig`](iqdb_types::IqdbError::InvalidConfig) from
/// construction, and
/// [`IqdbError::DimensionMismatch`](iqdb_types::IqdbError::DimensionMismatch) /
/// [`IqdbError::Duplicate`](iqdb_types::IqdbError::Duplicate) from insertion.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use iqdb_types::{DistanceMetric, VectorId};
/// # use iqdb_index::{Index, IndexCore, IndexStats};
/// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
/// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
/// # #[derive(Clone, Default)] struct FlatConfig;
/// # impl IndexCore for Flat {
/// #   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); } if self.rows.iter().any(|(e, _)| e == &id) { return Err(IqdbError::Duplicate); } self.rows.push((id, v)); Ok(()) }
/// #   fn delete(&mut self, id: &VectorId) -> Result<()> { match self.rows.iter().position(|(e, _)| e == id) { Some(p) => { let _ = self.rows.remove(p); Ok(()) } None => Err(IqdbError::NotFound) } }
/// #   fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) }
/// #   fn len(&self) -> usize { self.rows.len() }
/// #   fn dim(&self) -> usize { self.dim }
/// #   fn metric(&self) -> DistanceMetric { self.metric }
/// #   fn flush(&mut self) -> Result<()> { Ok(()) }
/// #   fn stats(&self) -> IndexStats { IndexStats { n_vectors: self.rows.len(), index_type: "flat", ..IndexStats::default() } }
/// # }
/// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { if dim == 0 { return Err(IqdbError::InvalidConfig { reason: "dim must be > 0" }); } Ok(Flat { dim, metric, rows: Vec::new() }) } }
/// # fn main() -> iqdb_types::Result<()> {
/// let index: Flat = iqdb_build::build(3, DistanceMetric::Cosine, vec![
///     (VectorId::from(10u64), Arc::from([1.0_f32, 0.0, 0.0].as_slice()), None),
///     (VectorId::from(20u64), Arc::from([0.0_f32, 1.0, 0.0].as_slice()), None),
/// ])?;
/// assert_eq!(index.len(), 2);
/// # Ok(()) }
/// ```
#[inline]
pub fn build<I, It>(dim: usize, metric: DistanceMetric, items: It) -> Result<I>
where
    I: Index,
    It: IntoIterator<Item = BuildItem>,
{
    IndexBuilder::<I>::new(dim, metric).build(items)
}

/// Bulk-insert a stream of vectors into an index you already hold, returning the
/// number inserted.
///
/// This is the incremental Tier-1 path: load an existing index, add more
/// vectors, keep it. It works over the object-safe
/// [`IndexCore`](iqdb_index::IndexCore) surface, so it accepts a concrete index
/// **or** a `&mut dyn IndexCore` trait object — the form the engine holds.
///
/// The items are inserted through
/// [`IndexCore::insert_batch`](iqdb_index::IndexCore::insert_batch), which a
/// backend may accelerate over a per-vector loop.
///
/// # Errors
///
/// - [`IqdbError::DimensionMismatch`](iqdb_types::IqdbError::DimensionMismatch)
///   if any vector's length differs from the index's `dim()`.
/// - [`IqdbError::Duplicate`](iqdb_types::IqdbError::Duplicate) if an id collides
///   with one already present (or repeated within the batch).
///
/// Insertion is fail-fast and **not transactional**: on error, items inserted
/// before the failing one remain in the index. The returned count is only
/// meaningful on `Ok`, where it equals the number of items supplied.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use iqdb_build::{build, build_into};
/// use iqdb_types::{DistanceMetric, VectorId};
/// # use iqdb_index::{Index, IndexCore, IndexStats};
/// # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
/// # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
/// # #[derive(Clone, Default)] struct FlatConfig;
/// # impl IndexCore for Flat {
/// #   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); } if self.rows.iter().any(|(e, _)| e == &id) { return Err(IqdbError::Duplicate); } self.rows.push((id, v)); Ok(()) }
/// #   fn delete(&mut self, id: &VectorId) -> Result<()> { match self.rows.iter().position(|(e, _)| e == id) { Some(p) => { let _ = self.rows.remove(p); Ok(()) } None => Err(IqdbError::NotFound) } }
/// #   fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) }
/// #   fn len(&self) -> usize { self.rows.len() }
/// #   fn dim(&self) -> usize { self.dim }
/// #   fn metric(&self) -> DistanceMetric { self.metric }
/// #   fn flush(&mut self) -> Result<()> { Ok(()) }
/// #   fn stats(&self) -> IndexStats { IndexStats { n_vectors: self.rows.len(), index_type: "flat", ..IndexStats::default() } }
/// # }
/// # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { if dim == 0 { return Err(IqdbError::InvalidConfig { reason: "dim must be > 0" }); } Ok(Flat { dim, metric, rows: Vec::new() }) } }
/// # fn main() -> iqdb_types::Result<()> {
/// // Start from an index, then append more vectors incrementally.
/// let mut index: Flat = build(2, DistanceMetric::Euclidean, vec![
///     (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
/// ])?;
/// let added = build_into(&mut index, vec![
///     (VectorId::from(2u64), Arc::from([1.0_f32, 0.0].as_slice()), None),
///     (VectorId::from(3u64), Arc::from([0.0_f32, 1.0].as_slice()), None),
/// ])?;
/// assert_eq!(added, 2);
/// assert_eq!(index.len(), 3);
/// # Ok(()) }
/// ```
#[inline]
pub fn build_into<I, It>(index: &mut I, items: It) -> Result<usize>
where
    I: IndexCore + ?Sized,
    It: IntoIterator<Item = BuildItem>,
{
    let batch: Vec<BuildItem> = items.into_iter().collect();
    let inserted = batch.len();
    index.insert_batch(batch)?;
    Ok(inserted)
}
