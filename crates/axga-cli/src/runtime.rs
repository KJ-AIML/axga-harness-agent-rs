//! Tokio runtime configuration - optimized for 1GB VPS.
//!
//! # Design (ADR-004)
//! - 2 worker threads (VPS typically has 1-2 vCPUs).
//! - 512 KB stack per worker thread.
//! - `panic = "abort"` for smaller binary.

/// Build a tokio runtime tuned for memory-constrained VPS.
pub fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .thread_stack_size(512 * 1024)
        .enable_all()
        .build()?)
}
