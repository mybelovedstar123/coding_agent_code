//! OpenAI-compatible chat completion client with bounded retries.

use crate::config;
use futures::future::BoxFuture;
use rand::Rng;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::time::Instant;

pub const PER_CALL_TIMEOUT_S: f64 = 75.0;
const RETRY_STATUS: [u16; 11] = [408, 409, 425, 429, 500, 502, 503, 504, 520, 522, 524];

#[derive(Debug, Clone)]
pub struct ModelError(pub String);

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

/// A token-usage block as the provider sent it (a JSON object; malformed fields count as zero where read).
pub type Usage = Map<String, Value>;

/// The first choice's message plus the response's usage and finish reason.
#[derive(Debug, Clone, Default)]
pub struct Reply {
    /// The whole message object as received (content, tool_calls, …).
    pub message: Map<String, Value>,
    pub content: Option<Value>,
    pub usage: Usage,
    pub finish_reason: Value,
}

impl Reply {
    pub fn tool_calls(&self) -> Option<&Value> {
        self.message.get("tool_calls")
    }
}

/// One chat completion. `tools` is the OpenAI tools array, or None; `effort` sets `reasoning.effort`.
pub trait ChatClient: Send + Sync {
    fn complete<'a>(
        &'a self,
        messages: Vec<Value>,
        tools: Option<&'a [Value]>,
        case_id: &'a str,
        base_url: Option<&'a str>,
        deadline: Instant,
        effort: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Reply, ModelError>>;
}

/// The request body the agent sends (the order of keys follows the reference).
pub fn request_body(messages: Vec<Value>, tools: Option<&[Value]>, effort: Option<&str>) -> Value {
    let mut body = Map::new();
    body.insert("model".into(), Value::String(config::model_name()));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("temperature".into(), json!(0.0));
    body.insert("seed".into(), json!(42));
    if let Some(e) = effort.filter(|e| !e.is_empty()) {
        body.insert("reasoning".into(), json!({"effort": e}));
    }
    if let Some(t) = tools.filter(|t| !t.is_empty()) {
        body.insert("tools".into(), Value::Array(t.to_vec()));
    }
    Value::Object(body)
}
