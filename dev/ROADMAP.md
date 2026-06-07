# iqdb-build -- Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

---

## v0.1.0 -- Scaffold (DONE)

Compiles, CI green, structure correct, no domain logic.

- [x] Manifest, README, CHANGELOG, REPS, license, CI, lints in place.
- [x] API surface sketched in `docs/API.md`.

---

## v0.2.0 -- `IndexBuilder<I>` + sequential build/build_into (DONE)

`IndexBuilder<I>` plus the Tier-1 `build` / `build_into` free functions; the full
sequential construction path. `build_into` is bound on the object-safe
`IndexCore`, so it also serves `&mut dyn IndexCore`.

Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested (completeness, equivalence, additivity,
  duplicate rejection).

---

## v0.3.0 -- rayon parallel construction + batching (DONE)

`IndexBuilder::build_parallel` splits the input into shards and constructs one
sub-index per shard on rayon's pool; `with_shards` / `shards` control the count.
rayon is a core dependency. Merging the shards into one index is deferred to 0.4
(per the split -> build-parallel -> merge design in DIRECTIVES §1).

Exit criteria:
- [x] New surface tested (completeness, clamping, id partitioning, single-shard
  parity) and benchmarked (`build_parallel` criterion group vs the sequential
  baseline).

---

## v0.4.0 -- index merging (`Mergeable`) + progress reporting + feature freeze (DONE)

The `Mergeable` trait (owned here, since `iqdb-index` is frozen at 1.0), the
`merge` free function, and `IndexBuilder::build_merged` for the full
split -> build-parallel -> merge pipeline. `on_progress` / `BuildProgress` report
shard completion.

Exit criteria:
- [x] No `todo!`/`unimplemented!`. Feature freeze declared (the public surface is
  complete; remaining work is `loom` hardening + stabilization).

---

## v0.5.0 -- concurrent-build correctness + API freeze (DONE)

`loom` model check (`tests/loom_iqdb_build.rs`) over the only shared-state path in
parallel construction — the progress counter — proving exact counting under every
interleaving. The public API is frozen for the rest of the 0.x series and until
2.0 once 1.0 ships.

### Frozen public API (recorded here)

- `VERSION: &str`
- `BuildItem` (type alias for `(VectorId, Arc<[f32]>, Option<Metadata>)`)
- free fns: `build`, `build_into`, `merge`
- `IndexBuilder<I>`: `new`, `with_config`, `with_shards`, `on_progress`, `dim`,
  `metric`, `config`, `shards`, `build`, `build_parallel`, `build_merged`; `Clone`
- `Mergeable` trait: `merge`
- `BuildProgress`: fields `shards_completed`, `shards_total`

Exit criteria:
- [x] Public API frozen (recorded above). `cargo audit` + `cargo deny` clean.

---

## v0.6.0 -- Alpha: real-backend integration (DONE)

Integration against the real `Index` backends now that they exist:
`tests/real_backends.rs` drives `build` / `build_parallel` / `build_into` against
`iqdb_flat::FlatIndex` and `iqdb_hnsw::HnswIndex` (path dev-deps), proving the
generic builder constructs, shards, appends to, and searches the actual indexes —
not just the in-crate toy. The public API is unchanged from the 0.5.0 freeze;
numbered 0.6.0 to align with the family version line.

- [x] Builds and searches real flat + HNSW indexes through the public surface.

---

## v0.7.x -> v0.9.x -- Beta -> RC

- 0.7.x: integrate against the `iqdb` engine; MINOR-compatible additions only.
- 0.8.x (beta): bug fixes; broader testing; final benchmarks.
- 0.9.x (rc): critical fixes + doc polish.

---

## v1.0.0 -- Stable

- [ ] Definition of Done (DIRECTIVES section 7) satisfied.
- [ ] Public API frozen until 2.0.
- [ ] Release note written; published to crates.io; tag pushed.

---

## Out of scope for 1.0

- The indexes themselves -- generic over them.
- Distributed build coordination -- reserved phase.
