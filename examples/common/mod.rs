//! A tiny brute-force `Index` shared by the examples.
//!
//! `iqdb-build` is generic over [`iqdb_index::Index`]; in real use you would
//! plug in `iqdb-flat`, `iqdb-hnsw`, or `iqdb-ivf`. The examples stay
//! dependency-light by defining the smallest faithful index here.

use std::sync::Arc;

use iqdb_build::Mergeable;
use iqdb_index::{Index, IndexCore, IndexStats};
use iqdb_types::{DistanceMetric, Hit, IqdbError, Metadata, Result, SearchParams, VectorId};

/// A brute-force flat index over `Euclidean`-style squared distance.
#[derive(Clone)]
pub struct Flat {
    dim: usize,
    metric: DistanceMetric,
    rows: Vec<(VectorId, Arc<[f32]>)>,
}

/// Unit configuration — nothing to tune for a flat scan.
#[derive(Clone, Default)]
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
            self.insert(id, vector, None)?;
        }
        Ok(())
    }
}
