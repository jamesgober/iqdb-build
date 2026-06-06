//! Integration tests for merging and the full build_merged pipeline.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{Flat, items};
use iqdb_build::{IndexBuilder, merge};
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, IqdbError, VectorId};

#[test]
fn merge_empty_is_none() {
    let merged = merge::<Flat>(Vec::new()).unwrap();
    assert!(merged.is_none());
}

#[test]
fn merge_single_returns_it() {
    let one: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .build(items(10, 2))
        .unwrap();
    let merged = merge(vec![one]).unwrap().unwrap();
    assert_eq!(merged.len(), 10);
}

#[test]
fn merge_reconstitutes_all_ids() {
    let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .with_shards(5)
        .build_parallel(items(250, 2))
        .unwrap();
    let merged = merge(shards).unwrap().unwrap();
    assert_eq!(merged.len(), 250);
}

#[test]
fn build_merged_is_complete() {
    let index: Flat = IndexBuilder::new(4, DistanceMetric::Euclidean)
        .with_shards(8)
        .build_merged(items(1_000, 4))
        .unwrap();
    assert_eq!(index.len(), 1_000);
    assert_eq!(index.dim(), 4);
}

#[test]
fn build_merged_matches_sequential_build() {
    let merged: Flat = IndexBuilder::new(3, DistanceMetric::Euclidean)
        .with_shards(7)
        .build_merged(items(300, 3))
        .unwrap();
    let sequential: Flat = IndexBuilder::new(3, DistanceMetric::Euclidean)
        .build(items(300, 3))
        .unwrap();

    assert_eq!(merged.len(), sequential.len());

    // Same searchable set: every id is its own nearest neighbour in both.
    let params = iqdb_types::SearchParams::new(1, DistanceMetric::Euclidean);
    for (id, v, _) in items(300, 3) {
        let a = merged.search(&v, &params).unwrap();
        let b = sequential.search(&v, &params).unwrap();
        assert_eq!(a[0].id, id);
        assert_eq!(b[0].id, id);
    }
}

#[test]
fn build_merged_empty_input() {
    let index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .build_merged(Vec::new())
        .unwrap();
    assert!(index.is_empty());
}

#[test]
fn cross_shard_duplicate_surfaces_at_merge() {
    // Two shards, each holding id 1 — the collision is only detectable when they
    // merge, not during the parallel build.
    let shard_a: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .build(vec![(
            VectorId::from(1u64),
            Arc::from([0.0_f32, 0.0].as_slice()),
            None,
        )])
        .unwrap();
    let shard_b: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .build(vec![(
            VectorId::from(1u64),
            Arc::from([1.0_f32, 0.0].as_slice()),
            None,
        )])
        .unwrap();

    let err = merge(vec![shard_a, shard_b]).unwrap_err();
    assert!(matches!(err, IqdbError::Duplicate));
}

#[test]
fn progress_fires_once_per_shard() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls2 = Arc::clone(&calls);
    let total_seen = Arc::new(AtomicUsize::new(0));
    let total_seen2 = Arc::clone(&total_seen);

    let _index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .with_shards(6)
        .on_progress(move |p| {
            total_seen2.store(p.shards_total, Ordering::Relaxed);
            let _ = calls2.fetch_add(1, Ordering::Relaxed);
        })
        .build_merged(items(600, 2))
        .unwrap();

    assert_eq!(calls.load(Ordering::Relaxed), 6);
    assert_eq!(total_seen.load(Ordering::Relaxed), 6);
}
