//! Integration tests for the sequential build surface: happy paths and the
//! error/edge cases the contract promises.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{Flat, FlatConfig, items};
use iqdb_build::{IndexBuilder, build, build_into};
use iqdb_index::IndexCore;
use iqdb_types::{DistanceMetric, IqdbError, VectorId};

#[test]
fn build_inserts_every_item() {
    let index: Flat = build(4, DistanceMetric::Euclidean, items(100, 4)).unwrap();
    assert_eq!(index.len(), 100);
    assert_eq!(index.dim(), 4);
    assert_eq!(index.metric(), DistanceMetric::Euclidean);
}

#[test]
fn build_empty_input_yields_empty_index() {
    let index: Flat = build(4, DistanceMetric::Cosine, Vec::new()).unwrap();
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
}

#[test]
fn builder_with_config_constructs() {
    let builder = IndexBuilder::<Flat>::with_config(8, DistanceMetric::Manhattan, FlatConfig);
    assert_eq!(builder.dim(), 8);
    assert_eq!(builder.metric(), DistanceMetric::Manhattan);
    assert_eq!(builder.config(), &FlatConfig);
    let index = builder.build(items(10, 8)).unwrap();
    assert_eq!(index.len(), 10);
}

#[test]
fn builder_is_reusable() {
    let builder = IndexBuilder::<Flat>::new(2, DistanceMetric::Euclidean);
    let a = builder.build(items(3, 2)).unwrap();
    let b = builder.build(items(5, 2)).unwrap();
    assert_eq!(a.len(), 3);
    assert_eq!(b.len(), 5);
}

#[test]
fn builder_clone_matches() {
    let builder = IndexBuilder::<Flat>::with_config(3, DistanceMetric::Cosine, FlatConfig);
    let cloned = builder.clone();
    assert_eq!(cloned.dim(), builder.dim());
    assert_eq!(cloned.metric(), builder.metric());
}

#[test]
fn build_into_is_additive() {
    let mut index: Flat = build(2, DistanceMetric::Euclidean, items(3, 2)).unwrap();
    let before = index.len();
    // Append three more items with non-colliding ids.
    let extra: Vec<_> = (3u64..6)
        .map(|i| {
            (
                VectorId::from(i),
                Arc::from([i as f32, 0.0].as_slice()),
                None,
            )
        })
        .collect();
    let added = build_into(&mut index, extra).unwrap();
    assert_eq!(added, 3);
    assert_eq!(index.len(), before + 3);
}

#[test]
fn build_into_works_through_trait_object() {
    let mut index: Flat = build(2, DistanceMetric::Euclidean, Vec::new()).unwrap();
    let dyn_index: &mut dyn IndexCore = &mut index;
    let added = build_into(dyn_index, items(4, 2)).unwrap();
    assert_eq!(added, 4);
    assert_eq!(index.len(), 4);
}

#[test]
fn dim_zero_is_invalid_config() {
    let err = build::<Flat, _>(0, DistanceMetric::Euclidean, Vec::new()).unwrap_err();
    assert!(matches!(err, IqdbError::InvalidConfig { .. }));
}

#[test]
fn dimension_mismatch_is_rejected() {
    // dim = 3 but the vector is length 2.
    let bad = vec![(
        VectorId::from(1u64),
        Arc::from([0.0_f32, 0.0].as_slice()),
        None,
    )];
    let err = build::<Flat, _>(3, DistanceMetric::Euclidean, bad).unwrap_err();
    assert!(matches!(
        err,
        IqdbError::DimensionMismatch {
            expected: 3,
            found: 2
        }
    ));
}

#[test]
fn duplicate_id_is_rejected() {
    let dup = vec![
        (
            VectorId::from(1u64),
            Arc::from([0.0_f32, 0.0].as_slice()),
            None,
        ),
        (
            VectorId::from(1u64),
            Arc::from([1.0_f32, 0.0].as_slice()),
            None,
        ),
    ];
    let err = build::<Flat, _>(2, DistanceMetric::Euclidean, dup).unwrap_err();
    assert!(matches!(err, IqdbError::Duplicate));
}

#[test]
fn build_into_is_not_transactional() {
    // The first item inserts; the second collides. The error surfaces, but the
    // first insert stays — build_into is fail-fast, not atomic.
    let mut index: Flat = build(2, DistanceMetric::Euclidean, Vec::new()).unwrap();
    let batch = vec![
        (
            VectorId::from(1u64),
            Arc::from([0.0_f32, 0.0].as_slice()),
            None,
        ),
        (
            VectorId::from(1u64),
            Arc::from([1.0_f32, 0.0].as_slice()),
            None,
        ),
    ];
    let err = build_into(&mut index, batch).unwrap_err();
    assert!(matches!(err, IqdbError::Duplicate));
    assert_eq!(index.len(), 1); // the first survived
}

#[test]
fn built_index_is_searchable() {
    // ids 0..5 at increasing coordinates; the origin's nearest is id 0.
    let index: Flat = build(2, DistanceMetric::Euclidean, items(5, 2)).unwrap();
    let hits = index
        .search(
            &[0.0, 0.0],
            &iqdb_types::SearchParams::new(1, DistanceMetric::Euclidean),
        )
        .unwrap();
    assert_eq!(hits[0].id, VectorId::from(0u64));
}
