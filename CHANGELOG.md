# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [0.6.0] - 2026-06-07

Alpha: entering the pre-1.0 validation band. The public API is unchanged from the
0.5.0 freeze — this release adds an end-to-end soak test and aligns the version
with the iQDB family line.

### Added

- `tests/consumer_simulation.rs` — an end-to-end soak test that drives the whole
  public surface the way the `iqdb` engine would (one-call build, configured
  build, parallel sharded build, `build_merged` with progress, incremental
  `build_into`, `merge`, and search/delete), asserting parallel-merged recall
  matches a sequential build. It runs against an in-crate index so the suite stays
  self-contained — cross-crate validation against the real `iqdb-flat` /
  `iqdb-hnsw` backends lives in `iqdb-eval` / the engine workspace, where all
  crates are present, not in this standalone crate's CI.

### Changed

- Numbered `0.6.0` to align with the iQDB family's version line (`iqdb-flat` /
  `iqdb-hnsw` are at `0.6.0`+; the public API remains frozen as committed at 0.5.0).

---

## [0.5.0] - 2026-06-06

Concurrent-build correctness and API freeze.

### Added

- `tests/loom_iqdb_build.rs` — a `loom` model check over the progress counter,
  the only shared-state path in parallel construction. It proves, across every
  interleaving, that the final shard count is exact and each completion is
  reported exactly once (no repeats, no gaps). Compiled only under `--cfg loom`.
- `[lints.rust]` registration of `cfg(loom)` so the gated test does not trip
  `unexpected_cfgs` under `-D warnings`.

### Changed

- **Public API frozen.** The surface is committed for the rest of the 0.x series
  and until 2.0 once 1.0 ships; the frozen list is recorded in `dev/ROADMAP.md`.

---

## [0.4.0] - 2026-06-06

Index merging, the full build pipeline, progress reporting, and feature freeze.

### Added

- `Mergeable` trait — a backend opts in by saying how to absorb another instance
  of itself (`fn merge(&mut self, other: Self) -> Result<()>`). The merge
  mechanism is the backend's (flat appends, IVF extends posting lists, graph
  re-runs boundary heuristics); the trait fixes only the observable result.
- `merge` free function — fold a `Vec<I>` of sub-indexes into one, returning
  `Ok(None)` for empty input. The natural companion to `build_parallel`.
- `IndexBuilder::build_merged` — the full bulk pipeline (split → build in
  parallel → merge) in one call, for any `I: Mergeable`.
- `IndexBuilder::on_progress` and the `BuildProgress` snapshot — register a
  `Send + Sync` callback invoked as each shard finishes building.
- Merge integration tests (completeness, sequential parity, cross-shard
  duplicate detection, empty input, progress firing once per shard), a merge
  equivalence property test, and a `merge` example.

### Changed

- **Feature freeze declared.** No new public surface lands before 1.0; the
  remaining work is `loom` hardening of the parallel path and stabilization.

---

## [0.3.0] - 2026-06-06

Parallel sharded construction.

### Added

- `IndexBuilder::build_parallel` — split the input into contiguous, near-equal
  shards and construct one sub-index per shard concurrently on rayon's
  work-stealing pool. Returns the sub-indexes in input order; merging them into a
  single index lands in 0.4.
- `IndexBuilder::with_shards` (immutable `#[must_use]` setter) and the
  `IndexBuilder::shards` accessor to control and read the shard count. The
  default is one shard per available CPU; the effective count is clamped to
  `1..=items.len()`.
- `rayon` as a core dependency (the crate's purpose is the parallel bulk path).
- Parallel integration tests (completeness, shard-count clamping, id
  partitioning without loss, single-shard parity with sequential build), a
  parallel completeness property test, a `parallel` example, and a criterion
  `build_parallel` benchmark group alongside the sequential baseline.

---

## [0.2.0] - 2026-06-06

The sequential construction path: the generic builder and its one-call shortcuts.

### Added

- `IndexBuilder<I>` — a configured, reusable, `Clone` plan that constructs any
  `iqdb_index::Index` (flat, HNSW, IVF, or a custom backend) from a stream of
  vectors. Constructors `new` (default config) and `with_config` (Tier-2
  tuning); accessors `dim`, `metric`, `config`; and `build`.
- `build` — Tier-1 free function: construct a fresh index from an
  `IntoIterator` of items in one call, using the backend's default config.
- `build_into` — Tier-1 free function: bulk-insert into an index you already
  hold, bound on the object-safe `IndexCore` so it accepts `&mut dyn IndexCore`.
- `BuildItem` type alias — the `(VectorId, Arc<[f32]>, Option<Metadata>)` tuple
  the index already consumes, so building never reshapes or re-copies data.
- `iqdb-index` and `iqdb-types` dependencies (both `1.0.0`).
- Property tests for the core invariants (completeness, equivalence with
  one-at-a-time insertion, additivity of `build_into`, duplicate rejection),
  integration tests for the error/edge cases, three runnable examples
  (`quickstart`, `incremental`, `configured`), and a criterion build-throughput
  baseline.
- `docs/API.md` filled out with the full v0.2 surface.

### Changed

- Removed the vestigial `std` / `serde` feature split: the crate builds on the
  std-only `iqdb-index`, so there is no `no_std` build to gate. The crate now
  has no feature flags.
- Added `Matt Callahan` to the crate authors.

---

## [0.1.0] - 2026-05-30

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.87.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/DIRECTIVES.md` and `dev/ROADMAP.md` (committed engineering standards + plan).

[Unreleased]: https://github.com/jamesgober/iqdb-build/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/jamesgober/iqdb-build/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jamesgober/iqdb-build/releases/tag/v0.5.0
