//! Ollama-compatible embedding client (local Ollama or the scorer's gateway).

use crate::config;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::Instant;

pub const BATCH: usize = 32;
/// The scored gateway forwards to a provider with a per-request token limit (observed: 120k tokens). Long records
/// (~16k chars each) must not be packed 32 to a request; cap each request by total characters as well.
pub const BATCH_MAX_CHARS: usize = 100_000;
const ATTEMPTS: usize = 8;
const RETRY_STATUS: [u16; 6] = [408, 429, 500, 502, 503, 504];

#[derive(Debug, Clone)]
pub enum EmbedError {
    /// The gateway rejected the request's size (or failed for a multi-text batch): retry with smaller batches.
    TooLarge(String),
    Failed(String),
}
