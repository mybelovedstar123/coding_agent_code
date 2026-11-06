//! OpenAI-compatible chat completion client with bounded retries.

use crate::config;
use futures::future::BoxFuture;
use rand::Rng;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::time::Instant;
