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

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::TooLarge(m) | EmbedError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for EmbedError {}

/// Unit vectors for texts; `deadline` bounds the whole call.
pub trait Embed: Send + Sync {
    fn embed<'a>(
        &'a self,
        texts: Vec<String>,
        deadline: Option<Instant>,
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, EmbedError>>;
}

pub struct Embedder {
    http: reqwest::Client,
    /// Bound concurrent embedding requests so a large seed wave cannot starve /run queries of gateway capacity.
    gate: Arc<Semaphore>,
}

impl Embedder {
    pub fn new(http: reqwest::Client) -> Self {
        Embedder {
            http,
            gate: Arc::new(Semaphore::new(4)),
        }
    }
}
