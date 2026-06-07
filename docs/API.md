# iqdb-build &mdash; API Reference

> Complete reference for **every** public item in `iqdb-build` as of **v1.0.0**:
> what it is, its parameters and return shape, the contract it carries, and
> worked examples for each use case.
>
> **Status: stable (1.0).** The public surface is committed under SemVer for the
> 1.x series — no breaking changes until 2.0 (the frozen surface is recorded in
> [`dev/ROADMAP.md`](../dev/ROADMAP.md)). Only additive, non-breaking changes are
> made within 1.x.

## Table of Contents

- [Overview](#overview)
- [The three tiers](#the-three-tiers)
- [Crate constants](#crate-constants)
  - [`VERSION`](#version)
- [Type aliases](#type-aliases)
  - [`BuildItem`](#builditem)
- [Tier 1 &mdash; the lazy path](#tier-1--the-lazy-path)
  - [`build`](#build)
  - [`build_into`](#build_into)
- [Tier 2 &mdash; the configured path](#tier-2--the-configured-path)
  - [`IndexBuilder`](#indexbuilder)
  - [`IndexBuilder::new`](#indexbuildernew)
  - [`IndexBuilder::with_config`](#indexbuilderwith_config)
  - [`IndexBuilder::dim` / `metric` / `config` / `shards`](#indexbuilderdim--metric--config--shards)
  - [`IndexBuilder::with_shards`](#indexbuilderwith_shards)
  - [`IndexBuilder::on_progress`](#indexbuilderon_progress)
  - [`IndexBuilder::build`](#indexbuilderbuild)
  - [`IndexBuilder::build_parallel`](#indexbuilderbuild_parallel)
  - [`IndexBuilder::build_merged`](#indexbuilderbuild_merged)
- [Progress](#progress)
  - [`BuildProgress`](#buildprogress)
- [Merging](#merging)
  - [`Mergeable`](#mergeable)
  - [`merge`](#merge)
- [Tier 3 &mdash; traits](#tier-3--traits)
- [Errors](#errors)
- [Feature flags](#feature-flags)
- [Concurrency surface](#concurrency-surface-planned)

---

## Overview

`iqdb-build` orchestrates index construction. Loading a million vectors into an
index one [`insert`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.IndexCore.html#tymethod.insert)
at a time is slow; this crate is the bulk path. It is **generic over**
[`iqdb_index::Index`](https://docs.rs/iqdb-index), so the same builder constructs
a flat, HNSW, or IVF index — or a backend of your own — without naming a concrete
type.

The crate adds **no error type and no data type of its own**. Build items are the
exact tuple the index already consumes, and every fallible call returns
[`iqdb_types::Result`](https://docs.rs/iqdb-types), propagating the backend's
errors unchanged. There is no `unsafe` in the crate, and it never panics on bad
input.

```rust
use std::sync::Arc;
use iqdb_build::IndexBuilder;
use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat {
#   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); } self.rows.push((id, v)); Ok(()) }
#   fn delete(&mut self, _id: &VectorId) -> Result<()> { Ok(()) }
#   fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> { Ok(Vec::new()) }
#   fn len(&self) -> usize { self.rows.len() }
#   fn dim(&self) -> usize { self.dim }
#   fn metric(&self) -> DistanceMetric { self.metric }
#   fn flush(&mut self) -> Result<()> { Ok(()) }
#   fn stats(&self) -> IndexStats { IndexStats::default() }
# }
# impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { Ok(Flat { dim, metric, rows: Vec::new() }) } }
# fn main() -> iqdb_types::Result<()> {
let index: Flat = IndexBuilder::new(3, DistanceMetric::Euclidean).build(vec![
    (VectorId::from(1u64), Arc::from([0.0_f32, 0.0, 0.0].as_slice()), None),
    (VectorId::from(2u64), Arc::from([1.0_f32, 0.0, 0.0].as_slice()), None),
])?;
assert_eq!(index.len(), 2);
# Ok(()) }
```

---

## The three tiers

The crate follows the iQDB tiered-API mandate.

| Tier | Surface | When |
|---|---|---|
| **Tier 1** | the free functions [`build`](#build) and [`build_into`](#build_into) | The common case: turn a batch of vectors into an index, or append a batch to one you hold. |
| **Tier 2** | [`IndexBuilder`](#indexbuilder) | Tuning the backend (its [`Config`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.Index.html#associatedtype.Config)), parallel sharded construction, merging, progress, or reusing one plan for many builds. |
| **Tier 3** | implementing [`iqdb_index::Index`] + [`iqdb_index::IndexCore`] (+ [`Mergeable`](#mergeable) to be mergeable) | Plugging in a brand-new backend; the same builder then constructs and merges it. |

[`iqdb_index::Index`]: https://docs.rs/iqdb-index/latest/iqdb_index/trait.Index.html
[`iqdb_index::IndexCore`]: https://docs.rs/iqdb-index/latest/iqdb_index/trait.IndexCore.html

---

## Crate constants

### `VERSION`

```rust
pub const VERSION: &str;
```

The crate's compile-time version (`CARGO_PKG_VERSION`), a `major.minor.patch`
SemVer core. Use it to report the exact `iqdb-build` a binary links against in
diagnostics and version-skew checks across the family.

```rust
let v = iqdb_build::VERSION;
assert_eq!(v.split('.').count(), 3);
assert!(v.split('.').all(|part| !part.is_empty()));
```

---

## Type aliases

### `BuildItem`

```rust
pub type BuildItem = (VectorId, Arc<[f32]>, Option<Metadata>);
```

One vector to feed into a build: its [`VectorId`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.VectorId.html),
its components owned as an [`Arc<[f32]>`](https://doc.rust-lang.org/std/sync/struct.Arc.html),
and its optional [`Metadata`](https://docs.rs/iqdb-types/latest/iqdb_types/struct.Metadata.html).

This is **exactly** the element type of
[`IndexCore::insert_batch`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.IndexCore.html#method.insert_batch),
so building never reshapes or re-copies your data: the `Arc` you supply is the
allocation the index stores. Vectors without metadata pass `None`.

- **`.0`** — the id naming this vector.
- **`.1`** — the components; its length must equal the index's `dim`.
- **`.2`** — optional metadata returned alongside hits, or `None`.

```rust
use std::sync::Arc;
use iqdb_build::BuildItem;
use iqdb_types::VectorId;

let item: BuildItem = (VectorId::from(7u64), Arc::from([0.1_f32, 0.2].as_slice()), None);
assert_eq!(item.0, VectorId::from(7u64));
assert_eq!(item.1.len(), 2);
```

---

## Tier 1 &mdash; the lazy path

Two free functions cover the whole common case. Both accept any
`IntoIterator<Item = BuildItem>` — a `Vec`, an array, or a lazy iterator.

### `build`

```rust
pub fn build<I, It>(dim: usize, metric: DistanceMetric, items: It) -> Result<I>
where
    I: Index,
    It: IntoIterator<Item = BuildItem>;
```

Construct a fresh index from a stream of vectors, using the backend's **default**
[`Config`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.Index.html#associatedtype.Config).
Shorthand for `IndexBuilder::new(dim, metric).build(items)`.

- **`dim`** — the dimensionality the index is built for.
- **`metric`** — the [`DistanceMetric`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.DistanceMetric.html)
  the index searches under.
- **`items`** — the vectors to insert.
- **Returns** the constructed index `I`, holding exactly the supplied vectors.

**Errors:** [`InvalidConfig`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html#variant.InvalidConfig)
from construction (e.g. `dim == 0`),
[`DimensionMismatch`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html#variant.DimensionMismatch)
if any vector's length differs from `dim`, and
[`Duplicate`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html#variant.Duplicate)
if two items share an id. Insertion is fail-fast; a failed `build` returns the
error and drops the half-built index, leaving nothing behind.

```rust
# use std::sync::Arc;
# use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if v.len()!=self.dim { return Err(IqdbError::DimensionMismatch{expected:self.dim,found:v.len()});} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{ if dim==0 {return Err(IqdbError::InvalidConfig{reason:"dim must be > 0"});} Ok(Flat{dim,metric,rows:Vec::new()}) } }
# fn main() -> iqdb_types::Result<()> {
let index: Flat = iqdb_build::build(2, DistanceMetric::Euclidean, vec![
    (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
    (VectorId::from(2u64), Arc::from([1.0_f32, 0.0].as_slice()), None),
])?;
assert_eq!(index.len(), 2);
# Ok(()) }
```

### `build_into`

```rust
pub fn build_into<I, It>(index: &mut I, items: It) -> Result<usize>
where
    I: IndexCore + ?Sized,
    It: IntoIterator<Item = BuildItem>;
```

Bulk-insert a stream of vectors into an index you **already hold**, returning the
number inserted. This is the incremental path: load an index, add more vectors,
keep it.

It is bound on the object-safe [`IndexCore`] surface (not [`Index`]), so it
accepts a concrete index **or** a `&mut dyn IndexCore` trait object — the form
the engine holds as `Box<dyn IndexCore>`.

- **`index`** — the index to insert into; its `dim`/`metric` are already fixed.
- **`items`** — the vectors to add.
- **Returns** the number of items inserted (equal to `items` length on `Ok`).

**Errors:** [`DimensionMismatch`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html#variant.DimensionMismatch)
and [`Duplicate`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html#variant.Duplicate)
as for [`build`]. Insertion is fail-fast and **not transactional**: on error,
items inserted before the failing one remain. The returned count is meaningful
only on `Ok`.

```rust
# use std::sync::Arc;
# use iqdb_build::{build, build_into};
# use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{ Ok(Flat{dim,metric,rows:Vec::new()}) } }
# fn main() -> iqdb_types::Result<()> {
let mut index: Flat = build(2, DistanceMetric::Euclidean, vec![
    (VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None),
])?;
let added = build_into(&mut index, vec![
    (VectorId::from(2u64), Arc::from([1.0_f32, 0.0].as_slice()), None),
])?;
assert_eq!(added, 1);
assert_eq!(index.len(), 2);

// Works through a trait object too.
let dyn_index: &mut dyn IndexCore = &mut index;
let added = build_into(dyn_index, vec![
    (VectorId::from(3u64), Arc::from([0.0_f32, 1.0].as_slice()), None),
])?;
assert_eq!(added, 1);
# Ok(()) }
```

---

## Tier 2 &mdash; the configured path

### `IndexBuilder`

```rust
pub struct IndexBuilder<I: Index> { /* private */ }

impl<I: Index> Clone for IndexBuilder<I>;
```

A configured, reusable plan for constructing an [`Index`]. It holds the three
things [`Index::new`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.Index.html#tymethod.new)
needs — the dimensionality, the [`DistanceMetric`], and the backend's own
[`Config`](https://docs.rs/iqdb-index/latest/iqdb_index/trait.Index.html#associatedtype.Config)
— and turns a stream of [`BuildItem`]s into a finished index.

It is **`Clone`** (when the backend's `Config` is, which the trait guarantees),
so one plan can build many indexes from different inputs. The same builder
constructs **any** backend implementing [`Index`].

### `IndexBuilder::new`

```rust
pub fn new(dim: usize, metric: DistanceMetric) -> Self;
```

Create a builder for a `dim`-dimensional index under `metric`, using the
backend's **default** `Config`. Use [`with_config`](#indexbuilderwith_config)
when the backend has parameters to tune.

```rust
# use iqdb_build::IndexBuilder;
# use iqdb_types::DistanceMetric;
# use std::sync::Arc;
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams, VectorId};
# struct Flat { dim: usize, metric: DistanceMetric }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self,_i:VectorId,_v:Arc<[f32]>,_m:Option<Metadata>)->Result<()>{Ok(())} fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{0} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric})} }
let builder = IndexBuilder::<Flat>::new(128, DistanceMetric::Cosine);
assert_eq!(builder.dim(), 128);
```

### `IndexBuilder::with_config`

```rust
pub fn with_config(dim: usize, metric: DistanceMetric, config: I::Config) -> Self;
```

Create a builder with an explicit backend `Config`. This is the Tier-2 tuning
entry point: pass the backend's own parameter struct (graph degree, cluster /
probe counts, …) and every index this builder produces is constructed with it.

```rust
# use iqdb_build::IndexBuilder;
# use iqdb_types::DistanceMetric;
# use std::sync::Arc;
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams, VectorId};
# struct Graph { dim: usize, metric: DistanceMetric, degree: usize }
# #[derive(Clone)] struct GraphConfig { degree: usize }
# impl Default for GraphConfig { fn default() -> Self { Self { degree: 16 } } }
# impl IndexCore for Graph { fn insert(&mut self,_i:VectorId,_v:Arc<[f32]>,_m:Option<Metadata>)->Result<()>{Ok(())} fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{0} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Graph { type Config = GraphConfig; fn new(dim:usize,metric:DistanceMetric,c:Self::Config)->Result<Self>{Ok(Graph{dim,metric,degree:c.degree})} }
let builder = IndexBuilder::<Graph>::with_config(64, DistanceMetric::Euclidean, GraphConfig { degree: 32 });
assert_eq!(builder.config().degree, 32);
```

### `IndexBuilder::dim` / `metric` / `config` / `shards`

```rust
pub fn dim(&self) -> usize;
pub fn metric(&self) -> DistanceMetric;
pub fn config(&self) -> &I::Config;
pub fn shards(&self) -> Option<usize>;
```

Read back the plan's settings. `dim` and `metric` return by value; `config`
borrows the backend config; `shards` returns the configured shard count for
[`build_parallel`](#indexbuilderbuild_parallel), or `None` when left to "auto"
(one shard per CPU).

### `IndexBuilder::with_shards`

```rust
#[must_use]
pub fn with_shards(self, shards: usize) -> Self;
```

Set the shard count used by [`build_parallel`](#indexbuilderbuild_parallel). The
default is "auto" — one shard per available CPU
([`std::thread::available_parallelism`]). Set it explicitly to cap parallelism,
to match the engine's shard layout, or for reproducible shard boundaries in
tests. A count of `0` is treated as `1`, and the effective count never exceeds
the item count (no empty shards). This is a consuming, immutable setter: it
returns a new builder, leaving the sequential [`build`](#indexbuilderbuild) path
unaffected.

```rust
# use iqdb_build::IndexBuilder;
# use iqdb_types::DistanceMetric;
# use std::sync::Arc;
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams, VectorId};
# struct Flat { dim: usize, metric: DistanceMetric }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self,_i:VectorId,_v:Arc<[f32]>,_m:Option<Metadata>)->Result<()>{Ok(())} fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{0} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric})} }
let builder = IndexBuilder::<Flat>::new(8, DistanceMetric::Euclidean).with_shards(4);
assert_eq!(builder.shards(), Some(4));
```

### `IndexBuilder::on_progress`

```rust
#[must_use]
pub fn on_progress<F>(self, callback: F) -> Self
where
    F: Fn(BuildProgress) + Send + Sync + 'static;
```

Register a callback invoked each time a shard finishes building during
[`build_parallel`](#indexbuilderbuild_parallel) and the build phase of
[`build_merged`](#indexbuilderbuild_merged). The callback receives a
[`BuildProgress`](#buildprogress). It must be `Send + Sync` (shards build
concurrently, so it may be called from several worker threads) and should be
cheap and non-blocking. The sequential [`build`](#indexbuilderbuild) does not
report progress. This is a consuming, immutable setter. See
[`BuildProgress`](#buildprogress) for a worked example.

### `IndexBuilder::build`

```rust
pub fn build<It>(&self, items: It) -> Result<I>
where
    It: IntoIterator<Item = BuildItem>;
```

Construct a fresh index with this builder's `dim`, `metric`, and `config`, then
bulk-insert every item. Same semantics and errors as the free
[`build`](#build) function (which is shorthand for
`IndexBuilder::new(dim, metric).build(items)`). The builder is `&self`, so it can
be called repeatedly to build many indexes.

```rust
# use std::sync::Arc;
# use iqdb_build::IndexBuilder;
# use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
# fn main() -> iqdb_types::Result<()> {
let builder = IndexBuilder::new(2, DistanceMetric::Euclidean);
let a: Flat = builder.build(vec![(VectorId::from(1u64), Arc::from([0.0_f32, 0.0].as_slice()), None)])?;
let b: Flat = builder.build(vec![(VectorId::from(9u64), Arc::from([1.0_f32, 1.0].as_slice()), None)])?;
assert_eq!(a.len(), 1);
assert_eq!(b.len(), 1);
# Ok(()) }
```

### `IndexBuilder::build_parallel`

```rust
pub fn build_parallel<It>(&self, items: It) -> Result<Vec<I>>
where
    It: IntoIterator<Item = BuildItem>,
    I::Config: Send + Sync;
```

Build the input as several independent sub-indexes — one per shard — in parallel.
The items are split into contiguous, near-equal shards and each shard is
constructed on rayon's work-stealing pool: every shard runs [`Index::new`] with
this builder's `dim`/`metric`/`config` and inserts its slice through
`insert_batch`. The returned `Vec` preserves shard order (shard 0 holds the first
slice of input).

Wall-clock build time drops roughly linearly with cores, because the shards share
nothing. Combining them into one index is the job of the merge phase (planned,
0.4); meanwhile the shards are directly usable by the engine, whose storage is
itself sharded.

- **`items`** — the vectors to build.
- **Returns** a `Vec<I>` of sub-indexes, one per shard, in input order.
- The shard count comes from [`with_shards`](#indexbuilderwith_shards), or is
  one-per-CPU when left to auto. It is clamped to `1..=items.len()`, so empty
  input yields a single empty index and you never get an empty shard otherwise.
- The extra bound **`I::Config: Send + Sync`** lets the config be shared across
  worker threads; the index type itself is already `Send + Sync` via the
  `IndexCore` supertrait bound.

**Errors:** the same construction and insertion errors as [`build`](#build),
surfaced from whichever shard hits them first. Duplicate detection is **per
shard**: two items with the same id in *different* shards are not caught here
(they would collide at merge time). Within a shard, a repeated id errors as usual.

```rust
# use std::sync::Arc;
# use iqdb_build::IndexBuilder;
# use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
# fn main() -> iqdb_types::Result<()> {
let items: Vec<_> = (0u64..1_000)
    .map(|i| (VectorId::from(i), Arc::from([i as f32, 0.0].as_slice()), None))
    .collect();
let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
    .with_shards(4)
    .build_parallel(items)?;
assert_eq!(shards.len(), 4);
assert_eq!(shards.iter().map(|s| s.len()).sum::<usize>(), 1_000);
# Ok(()) }
```

### `IndexBuilder::build_merged`

```rust
pub fn build_merged<It>(&self, items: It) -> Result<I>
where
    It: IntoIterator<Item = BuildItem>,
    I::Config: Send + Sync;
// requires I: Mergeable
```

The full bulk pipeline — *split → build in parallel → merge* — in one call. Runs
[`build_parallel`](#indexbuilderbuild_parallel) (so the shard count,
[`with_shards`](#indexbuilderwith_shards), and any
[`on_progress`](#indexbuilderon_progress) callback apply), then folds the shards
with [`merge`](#merge) into a single index. Available when the backend implements
[`Mergeable`](#mergeable).

- **`items`** — the vectors to build.
- **Returns** one index `I` holding the union of all shards.

**Errors:** any error from the parallel build, plus any from
[`Mergeable::merge`] — notably a **cross-shard** `Duplicate` (two shards holding
the same id), which the per-shard parallel build cannot see.

```rust
# use std::sync::Arc;
# use iqdb_build::{IndexBuilder, Mergeable};
# use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
# #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
# impl Mergeable for Flat { fn merge(&mut self, other: Self) -> Result<()> { for (id, v) in other.rows { self.insert(id, v, None)?; } Ok(()) } }
# fn main() -> iqdb_types::Result<()> {
let items: Vec<_> = (0u64..1_000)
    .map(|i| (VectorId::from(i), Arc::from([i as f32, 0.0].as_slice()), None))
    .collect();
let index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
    .with_shards(8)
    .build_merged(items)?;
assert_eq!(index.len(), 1_000);
# Ok(()) }
```

---

## Progress

### `BuildProgress`

```rust
pub struct BuildProgress {
    pub shards_completed: usize,
    pub shards_total: usize,
}
```

A progress snapshot delivered to the [`on_progress`](#indexbuilderon_progress)
callback as parallel construction proceeds. One snapshot is reported each time a
shard finishes, so `shards_completed` climbs toward `shards_total`. Because shards
build concurrently, snapshots may arrive on different threads and (rarely) out of
order; treat `shards_completed` as a monotonic high-water count for a progress
bar, not as a strict sequence.

**Derives / traits:** `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use iqdb_build::IndexBuilder;
use iqdb_types::{DistanceMetric, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams};
# struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
# fn main() -> iqdb_types::Result<()> {
let calls = Arc::new(AtomicUsize::new(0));
let calls2 = Arc::clone(&calls);
let items: Vec<_> = (0u64..100).map(|i| (VectorId::from(i), Arc::from([i as f32].as_slice()), None)).collect();
let _: Vec<Flat> = IndexBuilder::new(1, DistanceMetric::Euclidean)
    .with_shards(4)
    .on_progress(move |p| { assert_eq!(p.shards_total, 4); calls2.fetch_add(1, Ordering::Relaxed); })
    .build_parallel(items)?;
assert_eq!(calls.load(Ordering::Relaxed), 4);
# Ok(()) }
```

---

## Merging

### `Mergeable`

```rust
pub trait Mergeable: Index {
    fn merge(&mut self, other: Self) -> Result<()>;
}
```

A backend that can absorb another instance of itself. Implement it so
[`merge`](#merge) and [`build_merged`](#indexbuilderbuild_merged) can combine
sharded builds into one index. `other` is taken **by value** so an implementation
can move its storage rather than copy it.

**Contract:**

- **Same shape.** `other` must share `self`'s `dim` and `metric`; return
  `InvalidConfig` on a mismatch.
- **Set union.** After `self.merge(other)?`, every id searchable in either input
  is searchable in `self`. A cross-input id collision returns `Duplicate`.
- **Not transactional.** On `Err`, `self` may be left partially merged. Merge into
  a clone if you need the original back.

The *mechanism* is the backend's choice and differs per family — a flat index
appends rows, an IVF index extends posting lists, a graph index re-runs boundary
heuristics — which is exactly why it lives behind a trait rather than the
read-only `IndexCore` surface. See the [`iqdb_index` deletion/merge
notes](https://docs.rs/iqdb-index) for the per-family rationale.

```rust
# use std::sync::Arc;
# use iqdb_build::Mergeable;
# use iqdb_types::{DistanceMetric, IqdbError, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams};
# #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { if self.rows.iter().any(|(e,_)|e==&id){return Err(IqdbError::Duplicate);} self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
impl Mergeable for Flat {
    fn merge(&mut self, other: Self) -> Result<()> {
        if other.dim() != self.dim() || other.metric() != self.metric() {
            return Err(IqdbError::InvalidConfig { reason: "merge shape mismatch" });
        }
        for (id, vector) in other.rows {
            self.insert(id, vector, None)?;
        }
        Ok(())
    }
}
```

### `merge`

```rust
pub fn merge<I: Mergeable>(indexes: Vec<I>) -> Result<Option<I>>;
```

Fold a collection of sub-indexes into one by merging them in order. Returns
`Ok(None)` for empty input; otherwise merges every later index into the first and
returns `Ok(Some(_))`. The natural companion to
[`build_parallel`](#indexbuilderbuild_parallel).

**Errors:** the first [`Mergeable::merge`] error (e.g. a cross-shard `Duplicate`).
Not transactional — a partial fold is dropped on error.

```rust
# use std::sync::Arc;
# use iqdb_build::{IndexBuilder, Mergeable, merge};
# use iqdb_types::{DistanceMetric, IqdbError, VectorId};
# use iqdb_index::{Index, IndexCore, IndexStats};
# use iqdb_types::{Hit, Metadata, Result, SearchParams};
# #[derive(Clone)] struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
# #[derive(Clone, Default)] struct FlatConfig;
# impl IndexCore for Flat { fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> { self.rows.push((id,v)); Ok(()) } fn delete(&mut self,_i:&VectorId)->Result<()>{Ok(())} fn search(&self,_q:&[f32],_p:&SearchParams)->Result<Vec<Hit>>{Ok(Vec::new())} fn len(&self)->usize{self.rows.len()} fn dim(&self)->usize{self.dim} fn metric(&self)->DistanceMetric{self.metric} fn flush(&mut self)->Result<()>{Ok(())} fn stats(&self)->IndexStats{IndexStats::default()} }
# impl Index for Flat { type Config = FlatConfig; fn new(dim:usize,metric:DistanceMetric,_c:Self::Config)->Result<Self>{Ok(Flat{dim,metric,rows:Vec::new()})} }
# impl Mergeable for Flat { fn merge(&mut self, other: Self) -> Result<()> { for (id, v) in other.rows { self.insert(id, v, None)?; } Ok(()) } }
# fn main() -> iqdb_types::Result<()> {
let items: Vec<_> = (0u64..100).map(|i| (VectorId::from(i), Arc::from([i as f32].as_slice()), None)).collect();
let shards: Vec<Flat> = IndexBuilder::new(1, DistanceMetric::Euclidean).with_shards(4).build_parallel(items)?;
let one = merge(shards)?;
assert_eq!(one.map(|i| i.len()), Some(100));
# Ok(()) }
```

---

## Tier 3 &mdash; traits

Tier 3 is the [`iqdb_index::Index`] + [`iqdb_index::IndexCore`] pair: implement
them for a new backend and every construction function in this crate works on it
unchanged. To make the backend **mergeable** as well, also implement this crate's
[`Mergeable`](#mergeable) (it cannot live in the frozen `iqdb-index`, so
`iqdb-build` owns it). See the
[`iqdb-index` API reference](https://docs.rs/iqdb-index) for the index trait
contracts (ordering, deletion, concurrency).

---

## Errors

`iqdb-build` introduces no error type. Every fallible call returns
[`iqdb_types::Result<T>`](https://docs.rs/iqdb-types/latest/iqdb_types/type.Result.html),
whose error is the shared
[`IqdbError`](https://docs.rs/iqdb-types/latest/iqdb_types/enum.IqdbError.html)
(which implements [`error_forge::ForgeError`](https://docs.rs/error-forge)). The
variants the build surface can surface, all raised by the backend and propagated
unchanged:

| Variant | Raised when |
|---|---|
| `InvalidConfig { reason }` | [`build`](#build) / [`IndexBuilder::build`](#indexbuilderbuild) construction rejected the `dim`/`config` (e.g. `dim == 0`). |
| `DimensionMismatch { expected, found }` | An item's vector length differs from the index's `dim`. |
| `Duplicate` | An item's id collides with one already present, or is repeated within the batch. |

`IqdbError` is `#[non_exhaustive]`; a `match` on it must carry a wildcard arm.

---

## Feature flags

`iqdb-build` has **no** feature flags. It builds on `iqdb-index`, a pure,
std-only trait crate, and on `rayon` for parallel construction — the crate's
reason for being, so it is a core dependency rather than an optional toggle. The
default build is the whole surface.

---

## Concurrency model

The shards built by [`build_parallel`](#indexbuilderbuild_parallel) share nothing
— each constructs its own index from its own slice — so the only cross-thread
state in the crate is the `AtomicUsize` that counts shard completions and drives
the [`on_progress`](#indexbuilderon_progress) callback. As of v0.5.0 that path is
covered by a `loom` model check (`tests/loom_iqdb_build.rs`, compiled under
`--cfg loom`) that exhaustively verifies, over every interleaving, that the count
is exact and each completion is reported exactly once. The crate has no other
lock-free or shared-mutable path, and no `unsafe`.

The public API is **frozen** as of v0.5.0 (the full list is in the ROADMAP). The
0.6 → 0.9 series is consumer integration and stabilization, not new surface.

[`std::thread::available_parallelism`]: https://doc.rust-lang.org/std/thread/fn.available_parallelism.html
[`Mergeable::merge`]: #mergeable

---

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
