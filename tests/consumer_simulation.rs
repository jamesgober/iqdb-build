//! Consumer simulation: drive the whole public surface the way `iqdb` would, on
//! a realistic-ish dataset, and assert end-to-end correctness.
//!
//! This is the soak test that stands in for a real backend consumer until the
//! concrete index crates (`iqdb-flat`, `iqdb-hnsw`, `iqdb-ivf`) integrate
//! directly. It exercises: one-call build, configured build, parallel sharded
//! build, the full `build_merged` pipeline with progress, incremental
//! `build_into`, `merge`, and search/delete on the result — all through the
//! public API only.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{Flat, FlatConfig};
use iqdb_build::{IndexBuilder, build, build_into, merge};
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, Metadata, SearchParams, VectorId};

const DIM: usize = 16;

/// A reproducible dataset: id `i` sits at a point determined by `i`, so each
/// vector is its own exact nearest neighbour.
fn dataset(start: u64, n: u64) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (start..start + n)
        .map(|i| {
            let v: Vec<f32> = (0..DIM)
                .map(|d| ((i as f32) * 0.013 + (d as f32) * 1.7).sin())
                .collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}

/// The merged, parallel-built index must return exactly what a single sequential
/// build does — same membership, same nearest neighbour for every vector.
#[test]
fn parallel_merged_matches_sequential_recall() {
    let n = 3_000u64;

    let sequential: Flat = build(DIM, DistanceMetric::Euclidean, dataset(0, n)).unwrap();

    let merged: Flat = IndexBuilder::new(DIM, DistanceMetric::Euclidean)
        .with_shards(12)
        .build_merged(dataset(0, n))
        .unwrap();

    assert_eq!(merged.len(), sequential.len());
    assert_eq!(merged.len(), n as usize);

    let params = SearchParams::new(5, DistanceMetric::Euclidean);
    let mut checked = 0;
    for (id, v, _) in dataset(0, n).into_iter().step_by(50) {
        let seq_hits = sequential.search(&v, &params).unwrap();
        let merged_hits = merged.search(&v, &params).unwrap();
        // Exact backend: the nearest neighbour is the vector itself, identically
        // in both indexes.
        assert_eq!(seq_hits[0].id, id);
        assert_eq!(merged_hits[0].id, id);
        assert_eq!(seq_hits[0].id, merged_hits[0].id);
        checked += 1;
    }
    assert!(checked > 0);
}

/// A full ingestion lifecycle: bulk build, incrementally append a second batch,
/// then delete part of the first — observing the count and search at each step.
#[test]
fn ingestion_lifecycle() {
    // Bulk-load the first 2,000 via the configured builder.
    let mut index: Flat = IndexBuilder::with_config(DIM, DistanceMetric::Euclidean, FlatConfig)
        .build(dataset(0, 2_000))
        .unwrap();
    assert_eq!(index.len(), 2_000);

    // A second batch arrives; append it incrementally (non-overlapping ids).
    let added = build_into(&mut index, dataset(2_000, 1_000)).unwrap();
    assert_eq!(added, 1_000);
    assert_eq!(index.len(), 3_000);

    // Everything is searchable.
    let params = SearchParams::new(1, DistanceMetric::Euclidean);
    for (id, v, _) in dataset(0, 3_000).into_iter().step_by(250) {
        let hits = index.search(&v, &params).unwrap();
        assert_eq!(hits[0].id, id);
    }

    // Retire a slice of ids; deleted vectors must vanish from results.
    for i in (0u64..500).step_by(5) {
        index.delete(&VectorId::from(i)).unwrap();
    }
    assert_eq!(index.len(), 3_000 - 100);
    let gone = VectorId::from(0u64);
    let hits = index
        .search(
            &dataset(0, 1)[0].1,
            &SearchParams::new(1, DistanceMetric::Euclidean),
        )
        .unwrap();
    assert_ne!(hits[0].id, gone);
}

/// The staged form: build shards in parallel (engine keeps them sharded), then
/// optionally merge — both must preserve the full id set, with progress observed.
#[test]
fn sharded_then_merged_with_progress() {
    let n = 5_000u64;
    let calls = Arc::new(AtomicUsize::new(0));
    let calls2 = Arc::clone(&calls);

    let shards: Vec<Flat> = IndexBuilder::new(DIM, DistanceMetric::Euclidean)
        .with_shards(16)
        .on_progress(move |_| {
            let _ = calls2.fetch_add(1, Ordering::Relaxed);
        })
        .build_parallel(dataset(0, n))
        .unwrap();

    assert_eq!(shards.len(), 16);
    assert_eq!(calls.load(Ordering::Relaxed), 16);
    assert_eq!(shards.iter().map(IndexCore::len).sum::<usize>(), n as usize);

    let one = merge(shards).unwrap().unwrap();
    assert_eq!(one.len(), n as usize);
}
