//! Tier-1 quickstart: build an index from a batch of vectors in one call, then
//! search it.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example quickstart
//! ```

mod common;

use std::sync::Arc;

use common::Flat;
use iqdb_build::build;
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, SearchParams, VectorId};

fn main() -> iqdb_types::Result<()> {
    // A handful of 3-dimensional vectors. In practice these come from your
    // embedding model; here we spell them out.
    let vectors = [
        (1u64, [0.0_f32, 0.0, 0.0]),
        (2, [1.0, 0.0, 0.0]),
        (3, [0.0, 1.0, 0.0]),
        (4, [0.9, 0.1, 0.0]),
    ];

    // Shape them into build items: (id, Arc<[f32]>, optional metadata).
    let items = vectors
        .iter()
        .map(|(id, v)| (VectorId::from(*id), Arc::from(v.as_slice()), None));

    // One call constructs the index and inserts everything.
    let index: Flat = build(3, DistanceMetric::Euclidean, items)?;
    println!("built an index with {} vectors", index.len());

    // Search it: the two nearest neighbours of a point near vector 2.
    let hits = index.search(
        &[0.95, 0.05, 0.0],
        &SearchParams::new(2, DistanceMetric::Euclidean),
    )?;
    println!("nearest two to [0.95, 0.05, 0.0]:");
    for hit in &hits {
        println!("  id={:?}  distance={:.4}", hit.id, hit.distance);
    }

    Ok(())
}
