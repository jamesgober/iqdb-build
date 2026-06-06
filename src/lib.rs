//! # iqdb-build
//!
//! Bulk index construction for the **iqdb** embedded vector-database spine.
//! Loading a million vectors into an index one [`insert`](iqdb_index::IndexCore::insert)
//! at a time is slow; `iqdb-build` is the fast path. It is generic over the
//! [`iqdb_index::Index`] trait, so the same builder constructs flat, HNSW, and
//! IVF indexes without naming a concrete type.
//!
//! The crate exposes the three iQDB API tiers:
//!
//! - **Tier 1 — the lazy path.** The free functions [`build`] (construct a
//!   fresh index from a stream of vectors, with default configuration) and
//!   [`build_into`] (bulk-insert into an index you already hold, including a
//!   `&mut dyn IndexCore` trait object). One call each.
//! - **Tier 2 — the configured path.** [`IndexBuilder`], which carries the
//!   `dim`, `metric`, and the backend's own [`Config`](iqdb_index::Index::Config)
//!   so you can tune the index it constructs.
//!   [`build_parallel`](IndexBuilder::build_parallel) splits the input into
//!   shards and constructs them concurrently on rayon's pool;
//!   [`build_merged`](IndexBuilder::build_merged) folds those shards back into a
//!   single index — the full *split → build → merge* pipeline in one call. An
//!   optional [`on_progress`](IndexBuilder::on_progress) callback reports shard
//!   completion.
//! - **Tier 3 — the trait seam.** The [`iqdb_index::Index`] and
//!   [`iqdb_index::IndexCore`] traits, plus this crate's [`Mergeable`] (how a
//!   backend absorbs another instance of itself): implement them and the same
//!   builder constructs and merges your backend.
//!
//! Every fallible call returns [`iqdb_types::Result`]; errors raised by the
//! backend ([`Index::new`](iqdb_index::Index::new) and
//! [`insert_batch`](iqdb_index::IndexCore::insert_batch)) are propagated
//! unchanged. The crate adds no error type of its own and never panics on bad
//! input.
//!
//! ## Example
//!
//! ```
//! use std::sync::Arc;
//! use iqdb_build::IndexBuilder;
//! use iqdb_types::{DistanceMetric, VectorId};
//! # use iqdb_index::{Index, IndexCore, IndexStats};
//! # use iqdb_types::{Hit, IqdbError, Metadata, Result, SearchParams};
//! # struct Flat { dim: usize, metric: DistanceMetric, rows: Vec<(VectorId, Arc<[f32]>)> }
//! # #[derive(Clone, Default)] struct FlatConfig;
//! # impl IndexCore for Flat {
//! #   fn insert(&mut self, id: VectorId, v: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> {
//! #     if v.len() != self.dim { return Err(IqdbError::DimensionMismatch { expected: self.dim, found: v.len() }); }
//! #     if self.rows.iter().any(|(e, _)| e == &id) { return Err(IqdbError::Duplicate); }
//! #     self.rows.push((id, v)); Ok(())
//! #   }
//! #   fn delete(&mut self, id: &VectorId) -> Result<()> { match self.rows.iter().position(|(e, _)| e == id) { Some(p) => { let _ = self.rows.remove(p); Ok(()) } None => Err(IqdbError::NotFound) } }
//! #   fn search(&self, q: &[f32], p: &SearchParams) -> Result<Vec<Hit>> {
//! #     let mut h: Vec<Hit> = self.rows.iter().map(|(id, v)| Hit { id: id.clone(), distance: q.iter().zip(v.iter()).map(|(a, b)| (a - b) * (a - b)).sum(), metadata: None }).collect();
//! #     h.sort_by(|a, b| a.distance.total_cmp(&b.distance)); h.truncate(p.k); Ok(h)
//! #   }
//! #   fn len(&self) -> usize { self.rows.len() }
//! #   fn dim(&self) -> usize { self.dim }
//! #   fn metric(&self) -> DistanceMetric { self.metric }
//! #   fn flush(&mut self) -> Result<()> { Ok(()) }
//! #   fn stats(&self) -> IndexStats { IndexStats { n_vectors: self.rows.len(), index_type: "flat", ..IndexStats::default() } }
//! # }
//! # impl Index for Flat { type Config = FlatConfig; fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> { if dim == 0 { return Err(IqdbError::InvalidConfig { reason: "dim must be > 0" }); } Ok(Flat { dim, metric, rows: Vec::new() }) } }
//! # fn main() -> iqdb_types::Result<()> {
//! // Build a 3-dimensional index from three vectors in one call.
//! let items = vec![
//!     (VectorId::from(1u64), Arc::from([0.0_f32, 0.0, 0.0].as_slice()), None),
//!     (VectorId::from(2u64), Arc::from([1.0_f32, 0.0, 0.0].as_slice()), None),
//!     (VectorId::from(3u64), Arc::from([0.0_f32, 1.0, 0.0].as_slice()), None),
//! ];
//! let index: Flat = IndexBuilder::new(3, DistanceMetric::Euclidean).build(items)?;
//! assert_eq!(index.len(), 3);
//! # Ok(()) }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_must_use)]
#![deny(unused_results)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::unreachable)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![forbid(unsafe_code)]

mod builder;
mod merge;
mod parallel;

pub use crate::builder::{BuildItem, IndexBuilder, build, build_into};
pub use crate::merge::{Mergeable, merge};
pub use crate::parallel::BuildProgress;

/// The version of this crate, taken from `Cargo.toml` at compile time.
///
/// Exposed so a consumer can report the exact `iqdb-build` build it links
/// against — useful in diagnostics and version-skew checks across the iqdb
/// crate family.
///
/// # Examples
///
/// ```
/// // Carries a `major.minor.patch` SemVer core.
/// let version = iqdb_build::VERSION;
/// assert_eq!(version.split('.').count(), 3);
/// assert!(version.split('.').all(|part| !part.is_empty()));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
