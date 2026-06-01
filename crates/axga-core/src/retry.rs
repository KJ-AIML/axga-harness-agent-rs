//! HTTP retry with exponential backoff.
//!
//! Retries on 429 (rate limit) and 5xx (server errors).
//! Max 3 retries, base delay 1s, exponential backoff.

use axga_shared::error::{AxgaError, AxgaResult};
use std::time::Duration;
use tracing::warn;

/// Retry an async operation with exponential backoff.
pub async fn with_retry<F, Fut, T>(operation: F, max_retries: u32) -> AxgaResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = AxgaResult<T>>,
{
    let mut attempt = 0;
    let base_delay = Duration::from_secs(1);

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !is_retryable(&e) || attempt >= max_retries {
                    return Err(e);
                }
                attempt += 1;
                let delay = base_delay * 2u32.pow(attempt);
                warn!(attempt, delay_ms = delay.as_millis(), "retrying after error");
                tokio::time::sleep(delay).await;
            }
        }
    }
}

fn is_retryable(error: &AxgaError) -> bool {
    matches!(
        error,
        AxgaError::RateLimited(_)
            | AxgaError::Network(_)
            | AxgaError::Http { status: 500..=599, .. }
    )
}
