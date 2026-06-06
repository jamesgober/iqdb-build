//! Build throughput baselines: sequential vs parallel sharded construction.
//!
//! `bench_sequential` measures `iqdb_build::build` constructing one index from N
//! vectors. `bench_parallel` measures `IndexBuilder::build_parallel` building the
//! same N vectors as sharded sub-indexes across rayon's pool. The backend is a
//! minimal brute-force index whose `insert` does a small, representative amount
//! of per-vector float work, so the parallel numbers reflect real scaling rather
//! than a no-op push. Run with `cargo bench`.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use iqdb_build::{IndexBuilder, build};
use iqdb_index::{Index, IndexCore, IndexStats};
use iqdb_types::{DistanceMetric, Hit, IqdbError, Metadata, Result, SearchParams, VectorId};

struct Flat {
    dim: usize,
    metric: DistanceMetric,
    rows: Vec<(VectorId, Arc<[f32]>)>,
    checksum: f32,
}

#[derive(Clone, Default)]
struct FlatConfig;

impl IndexCore for Flat {
    fn insert(&mut self, id: VectorId, vector: Arc<[f32]>, _m: Option<Metadata>) -> Result<()> {
        // A little representative per-vector work so parallelism is visible.
        let norm: f32 = vector.iter().map(|x| x * x).sum();
        self.checksum += norm;
        self.rows.push((id, vector));
        Ok(())
    }
    fn delete(&mut self, _id: &VectorId) -> Result<()> {
        Ok(())
    }
    fn search(&self, _q: &[f32], _p: &SearchParams) -> Result<Vec<Hit>> {
        Ok(Vec::new())
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
        IndexStats::default()
    }
}

impl Index for Flat {
    type Config = FlatConfig;
    fn new(dim: usize, metric: DistanceMetric, _c: Self::Config) -> Result<Self> {
        if dim == 0 {
            return Err(IqdbError::InvalidConfig {
                reason: "dim must be > 0",
            });
        }
        Ok(Flat {
            dim,
            metric,
            rows: Vec::with_capacity(1024),
            checksum: 0.0,
        })
    }
}

fn make_items(n: usize, dim: usize) -> Vec<(VectorId, Arc<[f32]>, Option<Metadata>)> {
    (0..n)
        .map(|i| {
            let v: Vec<f32> = (0..dim).map(|d| (i as f32) + d as f32).collect();
            (VectorId::from(i as u64), Arc::from(v.as_slice()), None)
        })
        .collect()
}

fn bench_sequential(c: &mut Criterion) {
    let dim = 128;
    let mut group = c.benchmark_group("build_sequential");
    for &n in &[1_000usize, 10_000, 100_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || make_items(n, dim),
                |items| {
                    let index: Flat =
                        build(dim, DistanceMetric::Euclidean, items).expect("valid build");
                    black_box(index.len())
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_parallel(c: &mut Criterion) {
    let dim = 128;
    let mut group = c.benchmark_group("build_parallel");
    for &n in &[10_000usize, 100_000, 500_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || make_items(n, dim),
                |items| {
                    let shards: Vec<Flat> = IndexBuilder::new(dim, DistanceMetric::Euclidean)
                        .build_parallel(items)
                        .expect("valid build");
                    black_box(shards.iter().map(IndexCore::len).sum::<usize>())
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_sequential, bench_parallel);
criterion_main!(benches);
