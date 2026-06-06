//! Tier-2 configured build: use `IndexBuilder::with_config` to tune the backend,
//! and reuse one builder to construct several indexes.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example configured
//! ```

mod common;

use std::sync::Arc;

use common::{Flat, FlatConfig};
use iqdb_build::IndexBuilder;
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, Metadata, VectorId};

fn batch(start: u64, n: u64, dim: usize) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (start..start + n)
        .map(|i| {
            let v: Vec<f32> = (0..dim).map(|d| (i as f32) + d as f32).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}

fn main() -> iqdb_types::Result<()> {
    // A flat index has a unit config; a real backend would carry its own tuning
    // (graph degree, cluster/probe counts). `with_config` is where it goes.
    let builder = IndexBuilder::<Flat>::with_config(4, DistanceMetric::Cosine, FlatConfig);
    println!(
        "builder: dim={} metric={:?}",
        builder.dim(),
        builder.metric()
    );

    // One plan, many indexes — the builder is cheap to reuse.
    let shard_a = builder.build(batch(0, 500, 4))?;
    let shard_b = builder.build(batch(500, 500, 4))?;
    println!("shard_a: {} vectors", shard_a.len());
    println!("shard_b: {} vectors", shard_b.len());

    Ok(())
}
