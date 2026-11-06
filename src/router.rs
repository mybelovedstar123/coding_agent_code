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

pub fn catalog_text(tool_defs: &[Value]) -> String {
    tool_defs
        .iter()
        .map(|t| {
            let f = function_of(t);
            let desc = f
                .get("description")
                .filter(|d| crate::store::truthy(d))
                .map(py::str_value)
                .unwrap_or_default();
            let desc = py::squash_ws(&desc);
            let name = f.get("name").map(py::str_value).unwrap_or_else(|| "None".into());
            format!("- {name}: {}", py::head(&desc, 220))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

static JSON_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?s)\{.*\}"));

pub fn parse(text: &str, names: &HashSet<String>) -> Option<Decision> {
    let m = JSON_RE.search_str(text)?;
    let data: Value = serde_json::from_str(m).ok()?;
    let obj = data.as_object()?;
    let route = obj
        .get("route")
        .filter(|r| crate::store::truthy(r))
        .map(py::str_value)
        .unwrap_or_default()
        .to_lowercase();
    if !["tool", "memory", "chat"].contains(&route.as_str()) {
        return None;
    }
    let tools: Vec<String> = obj
        .get("tools")
        .and_then(Value::as_array)
        .map(|raw| {
            raw.iter()
                .filter_map(Value::as_str)
                .filter(|t| names.contains(*t) && !MEMORY_READ_TOOLS.contains(t))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    // A tool route that names no app tool is not actionable.
    let route = if route == "tool" && tools.is_empty() {
        "memory".to_string()
    } else {
        route
    };
    Some(Decision { route, tools })
}

fn messages(user_input: &str, tool_defs: &[Value], wire_system: &str) -> Vec<Value> {
    let host = if py::strip(wire_system).is_empty() {
        String::new()
    } else {
        format!(
            "\n\nHost instructions the assistant follows:\n{}",
            py::strip(wire_system)
        )
    };
    vec![
        json!({"role": "system", "content": format!("{ROUTER_PROMPT}\n\nTools:\n{}{host}", catalog_text(tool_defs))}),
        json!({"role": "user", "content": user_input}),
    ]
}
