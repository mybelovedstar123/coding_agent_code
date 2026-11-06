//! OpenAI-compatible chat completion client with bounded retries.

use crate::config;
use futures::future::BoxFuture;
use rand::Rng;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::time::Instant;

pub const PER_CALL_TIMEOUT_S: f64 = 75.0;
const RETRY_STATUS: [u16; 11] = [408, 409, 425, 429, 500, 502, 503, 504, 520, 522, 524];
