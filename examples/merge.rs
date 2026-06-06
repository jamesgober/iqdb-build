//! Full bulk pipeline: split a large input across cores, build the shards in
//! parallel, and merge them into a single index — with a live progress count.
//!
//! Run with:
//!
//! ```sh
//! cargo run --release --example merge
//! ```

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    let data = dataset(250_000, 128);
    let total = data.len();
    println!("building + merging {total} vectors...");

    let done = Arc::new(AtomicUsize::new(0));
    let done_cb = Arc::clone(&done);

    // split -> build in parallel -> merge, all in one call.
    let index: Flat = IndexBuilder::new(128, DistanceMetric::Euclidean)
        .with_shards(8)
        .on_progress(move |p| {
            let n = done_cb.fetch_add(1, Ordering::Relaxed) + 1;
            println!("  shard {n}/{} built", p.shards_total);
        })
        .build_merged(data)?;

    println!("merged index holds {} vectors", index.len());
    assert_eq!(index.len(), total);
    Ok(())
}
