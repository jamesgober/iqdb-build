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
        <strong>MSRV is 1.87+</strong> (Rust 2024 edition). Parallel bulk build. Incremental updates. Index merging.
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

<h2>What it does</h2>

- **Parallel build** &mdash; bulk inserts across CPU cores via rayon
- **Incremental** &mdash; load an existing index, add vectors, save
- **Merge** &mdash; combine trained indexes (segment builds, distributed builds)
- **Builder API** &mdash; a clean `IndexBuilder<I>` for users who don't want construction details
- **Progress** &mdash; optional progress reporting for long-running builds


<br>

## Installation

```toml
[dependencies]
iqdb-build = "0.1"
```

<br>

## Status

This is the <code>v0.1.0</code> scaffold: structure, tooling, and quality gates are in place; the implementation lands across the 0.x series per the <a href="./dev/ROADMAP.md"><code>ROADMAP</code></a> and <a href="./docs/API.md"><code>docs/API.md</code></a>.

<hr>
<br>

## Where It Fits

`iqdb-build` is a Phase-4 consumer of the index layer. It builds on:

- `iqdb-types` &mdash; core types
- `iqdb-index` &mdash; generic over any index
- `iqdb` &mdash; uses it for bulk ingestion

It is unblocked once `iqdb-index` exists.

<br>

## Contributing

See <a href="./dev/DIRECTIVES.md"><code>dev/DIRECTIVES.md</code></a> for engineering standards and the definition of done. Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

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
