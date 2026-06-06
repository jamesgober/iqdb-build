<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>iqdb-build</b>
    <br>
    <sub><sup>iQDB INDEX CONSTRUCTION</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/iqdb-build"><img alt="Crates.io" src="https://img.shields.io/crates/v/iqdb-build"></a>
    <a href="https://crates.io/crates/iqdb-build"><img alt="Downloads" src="https://img.shields.io/crates/d/iqdb-build?color=%230099ff"></a>
    <a href="https://docs.rs/iqdb-build"><img alt="docs.rs" src="https://img.shields.io/docsrs/iqdb-build"></a>
    <a href="https://github.com/jamesgober/iqdb-build/actions"><img alt="CI" src="https://github.com/jamesgober/iqdb-build/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.87%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        <strong>iqdb-build</strong> orchestrates high-throughput index construction: split input into chunks, build sub-indexes in parallel, merge them. Loading a million vectors one at a time is slow; this is the bulk path.
    </p>
    <p>
        It is generic over the `Index` trait, so the same builder works for flat, HNSW, and IVF.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.87+</strong> (Rust 2024 edition). One generic builder. Parallel sharded construction. Merge. Incremental append.
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

<h2>What it does</h2>

- **One-call build** &mdash; turn a batch of vectors into a finished index with [`build`](./docs/API.md#build)
- **Parallel** &mdash; split the input into shards and build them concurrently across CPU cores with [`build_parallel`](./docs/API.md#indexbuilderbuild_parallel) (rayon)
- **Merge** &mdash; fold sharded builds back into a single index with [`merge`](./docs/API.md#merge); [`build_merged`](./docs/API.md#indexbuilderbuild_merged) is the whole *split → build → merge* pipeline in one call
- **Incremental** &mdash; append more vectors to an index you already hold with [`build_into`](./docs/API.md#build_into), including a `&mut dyn IndexCore` trait object
- **Progress** &mdash; an optional [`on_progress`](./docs/API.md#indexbuilderon_progress) callback reports shard completion for long-running builds
- **Generic over the backend** &mdash; the same `IndexBuilder<I>` constructs flat, HNSW, IVF, or your own index — it never names a concrete type
- **Zero ceremony** &mdash; no error type and no data type of its own; build items are the tuple the index already consumes, and errors propagate unchanged

The feature set and the public API are frozen as of 0.5; what remains before 1.0 is integration against real consumers and stabilization. See the <a href="./dev/ROADMAP.md"><code>ROADMAP</code></a>.

<br>

## Installation

```toml
[dependencies]
iqdb-build = "0.5"
```

<br>

## Quick start

Shape your vectors into `(id, Arc<[f32]>, metadata)` tuples and build in one call. The same call constructs any backend that implements `iqdb_index::Index`:

```rust
use std::sync::Arc;
use iqdb_build::{build, build_into};
use iqdb_types::{DistanceMetric, VectorId};

// `MyIndex: iqdb_index::Index` — a flat, HNSW, or IVF index, or your own.
let items = vec![
    (VectorId::from(1u64), Arc::from([0.0_f32, 0.0, 0.0].as_slice()), None),
    (VectorId::from(2u64), Arc::from([1.0_f32, 0.0, 0.0].as_slice()), None),
    (VectorId::from(3u64), Arc::from([0.0_f32, 1.0, 0.0].as_slice()), None),
];
let mut index: MyIndex = build(3, DistanceMetric::Euclidean, items)?;

// Later, append more vectors without rebuilding.
let added = build_into(&mut index, more_items)?;
```

For tuning the backend, use the builder directly:

```rust
use iqdb_build::IndexBuilder;
use iqdb_types::DistanceMetric;

let builder = IndexBuilder::<MyIndex>::with_config(768, DistanceMetric::Cosine, my_config);
let index = builder.build(items)?;   // reuse `builder` for as many builds as you like
```

For large inputs, build across CPU cores and merge into one index — the whole pipeline in one call (the backend must implement [`Mergeable`](./docs/API.md#mergeable)):

```rust
use iqdb_build::IndexBuilder;
use iqdb_types::DistanceMetric;

let index: MyIndex = IndexBuilder::new(768, DistanceMetric::Cosine)
    .with_shards(8)                       // or leave it to auto (one per CPU)
    .on_progress(|p| eprintln!("{}/{}", p.shards_completed, p.shards_total))
    .build_merged(items)?;
```

To keep the shards separate (the engine's storage is itself sharded), use `build_parallel`, which returns `Vec<MyIndex>`.

Runnable end-to-end examples (with a toy backend) live in [`examples/`](./examples): `quickstart`, `incremental`, `configured`, `parallel`, and `merge`.

<br>

## Status

<code>v0.5.0</code> &mdash; **feature-complete, API frozen.** The generic `IndexBuilder<I>`; the Tier-1 `build` / `build_into` free functions; `build_parallel` for rayon-backed sharded construction; the `Mergeable` trait with `merge` and the one-call `build_merged` pipeline; and `on_progress` / `BuildProgress` reporting. Property-tested invariants (completeness, equivalence, additivity, duplicate rejection, parallel completeness, and merge equivalence), a `loom` model check over the concurrent progress path, five runnable examples, and a criterion harness comparing sequential and parallel build. Zero `unsafe`; every public item is documented with a runnable example. The public API is frozen (recorded in the <a href="./dev/ROADMAP.md"><code>ROADMAP</code></a>); the 0.6 → 0.9 series is consumer integration and stabilization toward a 1.0 that holds the surface until 2.0. Full reference in <a href="./docs/API.md"><code>docs/API.md</code></a>.

<hr>
<br>

## Where It Fits

`iqdb-build` is a Phase-4 consumer of the index layer. It builds on:

- `iqdb-index` &mdash; the `Index` / `IndexCore` traits it is generic over
- `iqdb-types` &mdash; the `VectorId`, `Metadata`, `DistanceMetric`, and `Result` vocabulary

and is consumed by:

- `iqdb` &mdash; for bulk ingestion

<br>

## Standards

Built to the iQDB Rust standard. See <a href="./REPS.md"><code>REPS.md</code></a> (Rust Efficiency &amp; Performance Standards) and <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for the engineering law and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>JAMES GOBER.</strong></sup>
</div>
