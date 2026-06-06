//! Incremental build: start from an index and append more vectors with
//! `build_into`, including through a `&mut dyn IndexCore` trait object.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example incremental
//! ```

mod common;

use std::sync::Arc;

use common::Flat;
use iqdb_build::{IndexBuilder, build_into};
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, Metadata, VectorId};

fn batch(ids: std::ops::Range<u64>) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    ids.map(|i| {
        let v = [i as f32, (i as f32) * 0.5];
        (VectorId::from(i), Arc::from(v.as_slice()), None)
    })
    .collect()
}

fn main() -> iqdb_types::Result<()> {
    // Build an initial index with the first batch.
    let mut index: Flat = IndexBuilder::new(2, DistanceMetric::Euclidean).build(batch(0..1_000))?;
    println!("initial: {} vectors", index.len());

    // Later, more data arrives. Append it without rebuilding from scratch.
    let added = build_into(&mut index, batch(1_000..1_500))?;
    println!("appended {added} more; total {}", index.len());

    // `build_into` also works through the object-safe `IndexCore` surface — the
    // form an engine holds as `Box<dyn IndexCore>`.
    let dyn_index: &mut dyn IndexCore = &mut index;
    let added = build_into(dyn_index, batch(1_500..2_000))?;
    println!("appended {added} via trait object; total {}", index.len());

    Ok(())
}
