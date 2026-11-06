//! Request router: one short model call that decides which agent path (which guidance text) handles the request. It
//! sees the request's system prompt, the user's message and the live tool catalog (names and descriptions, each
//! description shortened to 220 characters), and no memory records; its tool names are advisory and unused.

use crate::llm::{ChatClient, Usage};
use crate::prompts::ROUTER_PROMPT;
use crate::py;
use crate::pyre::Re;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::time::Instant;

/// Tools that only read the user's stored memories. Everything else in the catalog acts on an app, the outside
/// world, or the memory store itself.
const MEMORY_READ_TOOLS: [&str; 4] = [
    "search_memories",
    "fetch_memories",
    "search_subjects",
    "search_memories_in_subjects",
];

/// The router's decision: "tool", "memory" or "chat", and the app tools it named.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub route: String,
    pub tools: Vec<String>,
}

fn function_of(t: &Value) -> &Value {
    match t.get("function") {
        Some(f) if crate::store::truthy(f) => f,
        _ => t,
    }
}
