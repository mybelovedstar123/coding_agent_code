//! Ollama-compatible embedding client (local Ollama or the scorer's gateway).

use crate::config;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::Instant;

pub const BATCH: usize = 32;
