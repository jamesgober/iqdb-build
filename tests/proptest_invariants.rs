//! Property-based tests for the core build invariants (DIRECTIVES §8).
//!
//! These assert the guarantees that hold for *every* input, not just the
//! hand-picked cases in `build.rs`:
//!
//! - **Completeness** — building `n` unique, well-formed items yields an index
//!   of length `n`; nothing is dropped or duplicated.
//! - **Equivalence** — `build` is observationally identical to constructing the
//!   index and inserting the same items one at a time.
//! - **Additivity** — `build_into` raises `len()` by exactly the number of new
//!   items.
//! - **Duplicate rejection** — a repeated id always surfaces `Duplicate`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::Flat;
use iqdb_build::{build, build_into};
use iqdb_index::{Index, IndexCore};
use iqdb_types::{DistanceMetric, IqdbError, Metadata, VectorId};
use proptest::prelude::*;

/// Generate `n` unique items of `dim` dimensions with ids `0..n`.
fn unique_items(n: u64, dim: usize) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (0..n)
        .map(|i| {
            let v: Vec<f32> = (0..dim).map(|d| (i as f32) * 0.25 + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Completeness: every well-formed item lands exactly once.
    #[test]
    fn build_is_complete(n in 0u64..500, dim in 1usize..16) {
        let index: Flat = build(dim, DistanceMetric::Euclidean, unique_items(n, dim)).unwrap();
        prop_assert_eq!(index.len(), n as usize);
        prop_assert_eq!(index.dim(), dim);
    }

    /// Equivalence: `build` matches construct-then-insert-one-at-a-time.
    #[test]
    fn build_equals_manual_inserts(n in 0u64..300, dim in 1usize..12) {
        let built: Flat = build(dim, DistanceMetric::Euclidean, unique_items(n, dim)).unwrap();

        let mut manual = Flat::new(dim, DistanceMetric::Euclidean, Default::default()).unwrap();
        for (id, v, m) in unique_items(n, dim) {
            manual.insert(id, v, m).unwrap();
        }

        prop_assert_eq!(built.len(), manual.len());
        // Same searchable set: every id resolves as its own nearest neighbour in
        // both indexes.
        let params = iqdb_types::SearchParams::new(1, DistanceMetric::Euclidean);
        for (id, v, _) in unique_items(n, dim) {
            let a = built.search(&v, &params).unwrap();
            let b = manual.search(&v, &params).unwrap();
            prop_assert_eq!(&a[0].id, &id);
            prop_assert_eq!(&b[0].id, &id);
        }
    }

    /// Additivity: `build_into` raises `len()` by exactly the batch size.
    #[test]
    fn build_into_is_additive(initial in 0u64..200, extra in 0u64..200, dim in 1usize..8) {
        let mut index: Flat = build(dim, DistanceMetric::Euclidean, unique_items(initial, dim)).unwrap();
        let before = index.len();

        // Fresh ids that cannot collide with 0..initial.
        let more: Vec<_> = (initial..initial + extra)
            .map(|i| {
                let v: Vec<f32> = (0..dim).map(|d| (i as f32) + d as f32).collect();
                (VectorId::from(i), Arc::from(v.as_slice()), None)
            })
            .collect();

        let added = build_into(&mut index, more).unwrap();
        prop_assert_eq!(added, extra as usize);
        prop_assert_eq!(index.len(), before + extra as usize);
    }

    /// Parallel completeness: sharded build preserves every item, and the shard
    /// count obeys the `1..=len` clamp.
    #[test]
    fn parallel_build_is_complete(n in 0u64..500, dim in 1usize..8, req in 1usize..16) {
        use iqdb_build::IndexBuilder;
        let shards: Vec<Flat> = IndexBuilder::new(dim, DistanceMetric::Euclidean)
            .with_shards(req)
            .build_parallel(unique_items(n, dim))
            .unwrap();

        let total: usize = shards.iter().map(IndexCore::len).sum();
        prop_assert_eq!(total, n as usize);

        let expected_shards = req.min((n as usize).max(1));
        prop_assert_eq!(shards.len(), expected_shards);
    }

    /// Merge completeness: `build_merged` equals sequential `build` in size, for
    /// any shard count.
    #[test]
    fn build_merged_equals_sequential(n in 0u64..400, dim in 1usize..8, req in 1usize..16) {
        use iqdb_build::IndexBuilder;
        let merged: Flat = IndexBuilder::new(dim, DistanceMetric::Euclidean)
            .with_shards(req)
            .build_merged(unique_items(n, dim))
            .unwrap();
        let sequential: Flat = build(dim, DistanceMetric::Euclidean, unique_items(n, dim)).unwrap();
        prop_assert_eq!(merged.len(), sequential.len());
        prop_assert_eq!(merged.len(), n as usize);
    }

    /// Duplicate rejection: a repeated id always errors.
    #[test]
    fn duplicate_ids_rejected(n in 1u64..100, dim in 1usize..8) {
        let mut with_dup = unique_items(n, dim);
        // Repeat the first id at the end.
        let first = with_dup[0].clone();
        with_dup.push(first);

        let result: Result<Flat, _> = build(dim, DistanceMetric::Euclidean, with_dup);
        prop_assert!(matches!(result, Err(IqdbError::Duplicate)));
    }
}
