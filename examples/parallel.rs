//! Parallel sharded build: split a large input across CPU cores and construct
//! one sub-index per shard concurrently.
//!
//! Run with:
//!
//! ```sh
//! cargo run --release --example parallel
//! ```

mod common;

use std::sync::Arc;

use common::Flat;
use iqdb_build::IndexBuilder;
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, Metadata, VectorId};

fn dataset(n: u64, dim: usize) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (0..n)
        .map(|i| {
            let v: Vec<f32> = (0..dim).map(|d| (i as f32) * 0.01 + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}

fn main() -> iqdb_types::Result<()> {
    let data = dataset(100_000, 128);
    println!("building {} vectors as parallel shards...", data.len());

    // Auto shards: one per available CPU. Pass `.with_shards(n)` to override.
    let shards: Vec<Flat> =
        IndexBuilder::new(128, DistanceMetric::Euclidean).build_parallel(data)?;

    println!("built {} shards", shards.len());
    for (i, shard) in shards.iter().enumerate() {
        println!("  shard {i}: {} vectors", shard.len());
    }
    let total: usize = shards.iter().map(IndexCore::len).sum();
    println!("total: {total} vectors (merge into one index lands in v0.4)");

    Ok(())
}
