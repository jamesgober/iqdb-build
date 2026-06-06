//! Integration tests for parallel sharded construction.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use common::{Flat, items};
use iqdb_build::IndexBuilder;
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, VectorId};

#[test]
fn parallel_build_is_complete() {
    let shards: Vec<Flat> = IndexBuilder::new(4, DistanceMetric::Euclidean)
        .with_shards(8)
        .build_parallel(items(1_000, 4))
        .unwrap();
    assert_eq!(shards.len(), 8);
    let total: usize = shards.iter().map(IndexCore::len).sum();
    assert_eq!(total, 1_000);
}

#[test]
fn shard_count_is_capped_at_item_count() {
    // 3 items, 8 requested shards -> at most 3 non-empty shards.
    let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .with_shards(8)
        .build_parallel(items(3, 2))
        .unwrap();
    assert_eq!(shards.len(), 3);
    assert!(shards.iter().all(|s| s.len() == 1));
}

#[test]
fn empty_input_yields_one_empty_shard() {
    let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .with_shards(4)
        .build_parallel(Vec::new())
        .unwrap();
    assert_eq!(shards.len(), 1);
    assert!(shards[0].is_empty());
}

#[test]
fn single_shard_matches_sequential_build() {
    let parallel: Vec<Flat> = IndexBuilder::new(3, DistanceMetric::Euclidean)
        .with_shards(1)
        .build_parallel(items(50, 3))
        .unwrap();
    let sequential: Flat = IndexBuilder::new(3, DistanceMetric::Euclidean)
        .build(items(50, 3))
        .unwrap();
    assert_eq!(parallel.len(), 1);
    assert_eq!(parallel[0].len(), sequential.len());
}

#[test]
fn shards_partition_ids_without_loss() {
    // Every id 0..200 must appear in exactly one shard.
    let shards: Vec<Flat> = IndexBuilder::new(2, DistanceMetric::Euclidean)
        .with_shards(5)
        .build_parallel(items(200, 2))
        .unwrap();

    let params = iqdb_types::SearchParams::new(1, DistanceMetric::Euclidean);
    let mut found = [false; 200];
    for shard in &shards {
        for i in 0u64..200 {
            // A vector that exactly matches item i; if shard holds it, it is the
            // nearest with distance 0.
            let v: Vec<f32> = (0..2).map(|d| (i as f32) + (d as f32) * 0.5).collect();
            let hits = shard.search(&v, &params).unwrap();
            if let Some(hit) = hits.first() {
                if hit.id == VectorId::from(i) && hit.distance == 0.0 {
                    assert!(!found[i as usize], "id {i} appeared in two shards");
                    found[i as usize] = true;
                }
            }
        }
    }
    assert!(found.iter().all(|&f| f), "every id present exactly once");
}

#[test]
fn auto_shards_default_is_none_but_builds() {
    let builder = IndexBuilder::<Flat>::new(2, DistanceMetric::Euclidean);
    assert_eq!(builder.shards(), None);
    let shards = builder.build_parallel(items(64, 2)).unwrap();
    let total: usize = shards.iter().map(IndexCore::len).sum();
    assert_eq!(total, 64);
}
