//! `loom` model check for the only shared-state path in parallel construction:
//! the progress counter.
//!
//! `build_parallel` builds each shard in isolation — the shards share nothing —
//! so the sole cross-thread interaction is the `AtomicUsize` that counts shard
//! completions and drives the [`on_progress`] callback. rayon's pool cannot be
//! driven by `loom`, so this test models that counting discipline directly with
//! `loom`'s threads and atomics, exhaustively checking every interleaving:
//!
//! - the final count equals the number of shards, and
//! - each worker observes a **distinct** post-increment value in `1..=N`, so a
//!   `BuildProgress::shards_completed` is never reported twice or skipped.
//!
//! Compiled only under `--cfg loom` (see the `loom` CI job); a normal build sees
//! an empty crate.
//!
//! [`on_progress`]: iqdb_build::IndexBuilder::on_progress

#![cfg(loom)]
#![allow(clippy::unwrap_used)]

use loom::sync::Arc;
use loom::sync::atomic::{AtomicUsize, Ordering};

/// Mirrors `build_parallel`'s per-shard step: `fetch_add(1, Relaxed)` then report
/// the resulting count. With `N` shards the reports must be exactly the set
/// `1..=N`, in some order, with no repeats or gaps — under every interleaving.
fn model_progress_counter(n: usize) {
    loom::model(move || {
        let completed = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..n)
            .map(|_| {
                let completed = Arc::clone(&completed);
                loom::thread::spawn(move || {
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    assert!((1..=n).contains(&done), "count {done} out of range 1..={n}");
                    done
                })
            })
            .collect();

        let mut observed: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(
            completed.load(Ordering::Relaxed),
            n,
            "final count must equal shards"
        );
        observed.sort_unstable();
        assert_eq!(
            observed,
            (1..=n).collect::<Vec<_>>(),
            "each shard completion is reported exactly once, no gaps"
        );
    });
}

#[test]
fn progress_counter_is_exact_under_all_interleavings() {
    // Two threads keep the interleaving state space small enough for an
    // exhaustive loom search while still exercising the concurrent increment.
    model_progress_counter(2);
}
