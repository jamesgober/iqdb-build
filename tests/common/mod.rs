//! A minimal in-memory `Index` used to exercise the generic builder.
//!
//! `iqdb-build` is generic over [`iqdb_index::Index`]; the real backends
//! (`iqdb-flat`, `iqdb-hnsw`, `iqdb-ivf`) live in their own crates. To test the
//! builder in isolation we implement the smallest faithful index here: a
//! brute-force flat scan with true-removal deletion, duplicate rejection, and
//! dimension checks — enough to verify every invariant the builder relies on.

#![allow(dead_code)]

use std::sync::Arc;

use iqdb_build::Mergeable;
use iqdb_index::{Index, IndexCore, IndexStats};
use iqdb_types::{DistanceMetric, Hit, IqdbError, Metadata, Result, SearchParams, VectorId};

/// A brute-force flat index: stores every vector and scans on search.
#[derive(Debug, Clone)]
pub struct Flat {
    dim: usize,
    metric: DistanceMetric,
    rows: Vec<(VectorId, Arc<[f32]>)>,
}

/// Unit configuration — a flat index has nothing to tune.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct FlatConfig;

impl IndexCore for Flat {
    fn insert(
        &mut self,
        id: VectorId,
        vector: Arc<[f32]>,
        _metadata: Option<Metadata>,
    ) -> Result<()> {
        if vector.len() != self.dim {
            return Err(IqdbError::DimensionMismatch {
                expected: self.dim,
                found: vector.len(),
            });
        }
        if self.rows.iter().any(|(existing, _)| existing == &id) {
            return Err(IqdbError::Duplicate);
        }
        self.rows.push((id, vector));
        Ok(())
    }

    fn delete(&mut self, id: &VectorId) -> Result<()> {
        match self.rows.iter().position(|(existing, _)| existing == id) {
            Some(pos) => {
                let _ = self.rows.swap_remove(pos);
                Ok(())
            }
            None => Err(IqdbError::NotFound),
        }
    }

    fn search(&self, query: &[f32], params: &SearchParams) -> Result<Vec<Hit>> {
        if query.len() != self.dim {
            return Err(IqdbError::DimensionMismatch {
                expected: self.dim,
                found: query.len(),
            });
        }
        let mut hits: Vec<Hit> = self
            .rows
            .iter()
            .map(|(id, v)| Hit {
                id: id.clone(),
                distance: query
                    .iter()
                    .zip(v.iter())
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum(),
                metadata: None,
            })
            .collect();
        hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        hits.truncate(params.k);
        Ok(hits)
    }

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn metric(&self) -> DistanceMetric {
        self.metric
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn stats(&self) -> IndexStats {
        IndexStats {
            n_vectors: self.rows.len(),
            index_type: "flat",
            ..IndexStats::default()
        }
    }
}

impl Index for Flat {
    type Config = FlatConfig;

    fn new(dim: usize, metric: DistanceMetric, _config: Self::Config) -> Result<Self> {
        if dim == 0 {
            return Err(IqdbError::InvalidConfig {
                reason: "dim must be > 0",
            });
        }
        Ok(Flat {
            dim,
            metric,
            rows: Vec::new(),
        })
    }
}

impl Mergeable for Flat {
    fn merge(&mut self, other: Self) -> Result<()> {
        if other.dim != self.dim || other.metric != self.metric {
            return Err(IqdbError::InvalidConfig {
                reason: "merge shape mismatch",
            });
        }
        for (id, vector) in other.rows {
            // Re-inserting re-checks the dimension and duplicate invariants, so
            // a cross-shard id collision surfaces as `Duplicate` here.
            self.insert(id, vector, None)?;
        }
        Ok(())
    }
}

/// Build `n` deterministic `dim`-dimensional items with ids `0..n`.
pub fn items(n: u64, dim: usize) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (0..n)
        .map(|i| {
            let v: Vec<f32> = (0..dim).map(|d| (i as f32) + (d as f32) * 0.5).collect();
            (VectorId::from(i), Arc::from(v.as_slice()), None)
        })
        .collect()
}
