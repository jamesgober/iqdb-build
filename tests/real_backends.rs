//! Integration against the real `Index` backends.
//!
//! The other test files drive a toy in-crate `Flat` to keep the unit suite
//! self-contained. This one proves the generic builder constructs the **actual**
//! iQDB indexes — `iqdb_flat::FlatIndex` and `iqdb_hnsw::HnswIndex` — through the
//! same `build` / `build_parallel` / `build_into` surface, then searches the
//! results to confirm they are usable indexes. This is the v0.6.0 alpha
//! integration milestone (roadmap §0.6: integrate against real consumers).
//!
//! The backends are path dev-dependencies; they are dropped from the published
//! manifest, so this coupling is test-only.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::sync::Arc;

use iqdb_build::{IndexBuilder, build, build_into};
use iqdb_flat::{FlatConfig, FlatIndex};
use iqdb_hnsw::{HnswConfig, HnswIndex};
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, Metadata, SearchParams, VectorId};

const DIM: usize = 8;

fn items(n: u64) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (0..n)
        .map(|i| {
            let v: Vec<f32> = (0..DIM).map(|d| (i as f32) * 0.1 + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}

#[test]
fn build_constructs_a_real_flat_index() {
    let index: FlatIndex = build(DIM, DistanceMetric::Euclidean, items(500)).unwrap();
    assert_eq!(index.len(), 500);

    // Item 0 sits at its own coordinates, so it is its own nearest neighbour.
    let q: Vec<f32> = (0..DIM).map(|d| d as f32).collect();
    let hits = index
        .search(&q, &SearchParams::new(1, DistanceMetric::Euclidean))
        .unwrap();
    assert_eq!(hits[0].id, VectorId::from(0u64));
}

#[test]
fn build_constructs_a_real_hnsw_index() {
    let index: HnswIndex = build(DIM, DistanceMetric::Euclidean, items(500)).unwrap();
    assert_eq!(index.len(), 500);

    let q: Vec<f32> = (0..DIM).map(|d| d as f32).collect();
    let hits = index
        .search(&q, &SearchParams::new(5, DistanceMetric::Euclidean))
        .unwrap();
    assert!(!hits.is_empty());
    // The exact nearest should be found at this scale with the default beam.
    assert_eq!(hits[0].id, VectorId::from(0u64));
}

#[test]
fn configured_build_threads_the_backend_config() {
    // Flat: unit config.
    let flat = IndexBuilder::<FlatIndex>::with_config(DIM, DistanceMetric::Cosine, FlatConfig)
        .build(items(100))
        .unwrap();
    assert_eq!(flat.len(), 100);

    // HNSW: a real tuning config flows through unchanged.
    let cfg = HnswConfig::default().with_ef_search(128).with_m(24);
    let builder = IndexBuilder::<HnswIndex>::with_config(DIM, DistanceMetric::Euclidean, cfg);
    let hnsw = builder.build(items(100)).unwrap();
    assert_eq!(hnsw.len(), 100);
}

#[test]
fn build_parallel_shards_real_backends() {
    let flat_shards: Vec<FlatIndex> = IndexBuilder::new(DIM, DistanceMetric::Euclidean)
        .with_shards(4)
        .build_parallel(items(1_000))
        .unwrap();
    assert_eq!(flat_shards.len(), 4);
    assert_eq!(flat_shards.iter().map(IndexCore::len).sum::<usize>(), 1_000);

    let hnsw_shards: Vec<HnswIndex> = IndexBuilder::new(DIM, DistanceMetric::Euclidean)
        .with_shards(4)
        .build_parallel(items(1_000))
        .unwrap();
    assert_eq!(hnsw_shards.len(), 4);
    assert_eq!(hnsw_shards.iter().map(IndexCore::len).sum::<usize>(), 1_000);
}

#[test]
fn build_into_appends_to_real_backends() {
    // Flat, incrementally.
    let mut flat: FlatIndex = build(DIM, DistanceMetric::Euclidean, items(100)).unwrap();
    let extra: Vec<_> = (100u64..150)
        .map(|i| {
            let v: Vec<f32> = (0..DIM).map(|d| (i as f32) + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect();
    let added = build_into(&mut flat, extra).unwrap();
    assert_eq!(added, 50);
    assert_eq!(flat.len(), 150);

    // And through the object-safe surface, as the engine holds it.
    let mut hnsw: HnswIndex = build(DIM, DistanceMetric::Euclidean, items(100)).unwrap();
    let dyn_index: &mut dyn IndexCore = &mut hnsw;
    let more: Vec<_> = (100u64..130)
        .map(|i| {
            let v: Vec<f32> = (0..DIM).map(|d| (i as f32) + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect();
    let added = build_into(dyn_index, more).unwrap();
    assert_eq!(added, 30);
    assert_eq!(hnsw.len(), 130);
}
