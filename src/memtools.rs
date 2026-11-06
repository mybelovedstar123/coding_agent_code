//! The five memory tools, served from the local store.
//!
//! The request catalog carries these tools; the harness answers them from the user's seeded store it holds (the
//! calls are also sent to `tool_endpoint`). Arguments are parsed tolerantly so a model that sends a single string
//! where a list is expected still gets a useful result.

use crate::excerpt::{excerpt, Idf};
use crate::py;
use crate::pytool;
use crate::store::{cmp_content, fmt_ts, render_pair, truthy, Pair, Store, UserMemory};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;
use tokio::time::Instant;

pub const MEMORY_TOOLS: [&str; 5] = [
    "save_memory",
    "search_memories",
    "fetch_memories",
    "search_subjects",
    "search_memories_in_subjects",
];

pub const READ_TOOLS: [&str; 4] = [
    "search_memories",
    "fetch_memories",
    "search_subjects",
    "search_memories_in_subjects",
];

pub const PREVIEW_CHARS: usize = 900;
/// Above any stored record; a longer one is marked as shortened.
pub const FETCH_CHARS: usize = 60_000;

pub fn is_memory_tool(name: &str) -> bool {
    MEMORY_TOOLS.contains(&name)
}

pub fn is_read_tool(name: &str) -> bool {
    READ_TOOLS.contains(&name)
}

/// Fallback schemas, used only when the wire catalog omits a memory tool (name, description and parameters, in
/// the reference's order).
pub static FALLBACK_DEFS: LazyLock<Vec<(&'static str, Value)>> = LazyLock::new(|| {
    vec![
        (
            "search_memories",
            json!({
                "description": "Search past conversations and return compact memory summaries. Use fetch_memories for selected IDs that need full text.",
                "parameters": {
                    "type": "object",
                    "required": ["queries"],
                    "properties": {"queries": {"type": "array", "items": {"type": "string"}}, "topK": {"type": "integer"}},
                },
            }),
        ),
        (
            "fetch_memories",
            json!({
                "description": "Fetch full conversation text for selected memory pair IDs.",
                "parameters": {
                    "type": "object",
                    "required": ["pairIds"],
                    "properties": {"pairIds": {"type": "array", "items": {"type": "string"}}},
                },
            }),
        ),
        (
            "search_subjects",
            json!({
                "description": "Search the user's subject graph and return subject objects with id, name, description, and similarity.",
                "parameters": {
                    "type": "object",
                    "required": ["queries"],
                    "properties": {"queries": {"type": "array", "items": {"type": "string"}}, "topK": {"type": "integer"}},
                },
            }),
        ),
        (
            "search_memories_in_subjects",
            json!({
                "description": "Semantic memory search inside one subject. Use focused queries; fetch selected IDs for full text.",
                "parameters": {
                    "type": "object",
                    "required": ["subject_id", "queries"],
                    "properties": {
                        "subject_id": {"type": "string"},
                        "queries": {"type": "array", "items": {"type": "string"}},
                        "topK": {"type": "integer"},
                    },
                },
            }),
        ),
    ]
});

fn list_arg(args: &Map<String, Value>, names: &[&str]) -> Vec<String> {
    for name in names {
        match args.get(*name) {
            Some(Value::String(s)) if !py::strip(s).is_empty() => return vec![s.clone()],
            Some(Value::Array(items)) => {
                return items
                    .iter()
                    .map(py::str_value)
                    .filter(|v| !py::strip(v).is_empty())
                    .collect();
            }
            _ => {}
        }
    }
    Vec::new()
}

/// `int(args.get(name) or default)` clamped to [lo, hi]; an unparseable value gives the default.
fn int_arg(args: &Map<String, Value>, name: &str, default: i64, lo: i64, hi: i64) -> i64 {
    let raw = args.get(name).filter(|v| truthy(v));
    let parsed: Option<i64> = match raw {
        None => Some(default),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Some(Value::Bool(b)) => Some(*b as i64),
        Some(Value::String(s)) => py::strip(s).replace('_', "").parse::<i64>().ok(),
        Some(_) => None,
    };
    parsed.map(|v| v.clamp(lo, hi)).unwrap_or(default)
}

fn clip(text: &str, limit: usize) -> String {
    let text = py::strip(text);
    if py::len(text) <= limit {
        return text.to_string();
    }
    let head = limit * 2 / 3;
    format!(
        "{} \u{2026}[truncated]\u{2026} {}",
        py::head(text, head),
        py::tail(text, limit - head)
    )
}

fn preview(pair: &Pair, query: &str, idf: Option<&Idf>) -> Value {
    let full = render_pair(pair);
    let body = excerpt(&full, query, PREVIEW_CHARS, idf);
    let mut out = Map::new();
    out.insert("id".into(), json!(pair.pair_id));
    out.insert("timestamp".into(), json!(fmt_ts(&pair.ts)));
    out.insert("preview".into(), json!(body));
    if py::len(&body) < py::len(&full) {
        out.insert("excerpted".into(), json!(true));
    }
    Value::Object(out)
}

fn sorted_previews(mut previews: Vec<Value>) -> Value {
    previews.sort_by(|a, b| a["timestamp"].as_str().cmp(&b["timestamp"].as_str()));
    json!({"memories": previews})
}
