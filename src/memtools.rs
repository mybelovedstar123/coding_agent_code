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

/// The Python exception class name of an embedding failure, as the reference reports it.
fn embed_error_name(e: crate::embed::EmbedError) -> String {
    match e {
        crate::embed::EmbedError::TooLarge(_) => "RequestTooLarge".into(),
        crate::embed::EmbedError::Failed(_) => "EmbedError".into(),
    }
}

fn error(msg: impl Into<String>) -> Value {
    json!({"error": msg.into()})
}

/// Runs one memory tool. `Err` carries the type name of a failure (an embedding error), which the caller reports.
pub async fn execute(
    store: &Store,
    user_id: &str,
    name: &str,
    args: &Map<String, Value>,
    deadline: Option<Instant>,
) -> Result<Value, String> {
    match name {
        "search_memories" => {
            let queries = list_arg(args, &["queries", "query", "q"]);
            if queries.is_empty() {
                return Ok(error("at least one non-empty query is required"));
            }
            let session = args.get("sessionId").filter(|v| truthy(v)).map(py::str_value);
            let hits = store
                .search(
                    user_id,
                    &queries,
                    int_arg(args, "topK", 8, 1, 25) as usize,
                    None,
                    session.as_deref(),
                    deadline,
                )
                .await
                .map_err(embed_error_name)?;
            let mem = store.user(user_id);
            let mem = mem.read().await;
            let idf = mem.idf();
            let q = queries.join(" ");
            Ok(sorted_previews(
                hits.iter().map(|(p, _)| preview(p, &q, Some(&idf))).collect(),
            ))
        }
        "fetch_memories" => {
            let ids = list_arg(args, &["pairIds", "pair_ids", "ids", "id"]);
            let mem = store.user(user_id);
            let mem = mem.read().await;
            let mut out = Vec::new();
            for pid in ids {
                let Some(pair) = mem.pairs.get(&pid) else { continue };
                let mut item = Map::new();
                item.insert("id".into(), json!(pair.pair_id));
                item.insert("timestamp".into(), json!(fmt_ts(&pair.ts)));
                item.insert("user".into(), json!(clip(&pair.prompt, FETCH_CHARS)));
                item.insert("ditto".into(), json!(clip(&pair.response, FETCH_CHARS)));
                let full = py::len(&pair.prompt) + py::len(&pair.response);
                if full > FETCH_CHARS {
                    item.insert(
                        "note".into(),
                        json!(format!(
                            "record is {full} characters and was shortened here; the complete text is available to \
                             {} via records([{}])",
                            pytool::TOOL_NAME,
                            py::repr_str(&pair.pair_id)
                        )),
                    );
                }
                out.push(Value::Object(item));
            }
            Ok(json!({"memories": out}))
        }
        "search_subjects" => {
            let queries = list_arg(args, &["queries", "query", "q"]);
            if queries.is_empty() {
                return Ok(error("at least one non-empty query is required"));
            }
            let hits = store
                .search_subjects(user_id, &queries, int_arg(args, "topK", 8, 1, 25) as usize, None)
                .await
                .map_err(embed_error_name)?;
            let subjects: Vec<Value> = hits
                .iter()
                .map(|(s, score)| {
                    json!({
                        "id": s.subject_id,
                        "name": s.text,
                        "description": s.description,
                        "memoryCount": s.pairs.len(),
                        "similarity": py::float_value(py::round(*score, 4)),
                    })
                })
                .collect();
            Ok(json!({"subjects": subjects}))
        }
        "search_memories_in_subjects" => {
            let sid = args
                .get("subject_id")
                .filter(|v| truthy(v))
                .or_else(|| args.get("subjectId").filter(|v| truthy(v)))
                .map(py::str_value)
                .unwrap_or_default();
            let (queries, restrict) = {
                let mem = store.user(user_id);
                let mem = mem.read().await;
                let Some(subj) = mem.subjects.get(&sid) else {
                    return Ok(error(format!(
                        "unknown subject id {}; call search_subjects first",
                        py::repr_str(&sid)
                    )));
                };
                let mut queries = list_arg(args, &["queries", "query", "q"]);
                if queries.is_empty() {
                    queries = vec![subj.text.clone()];
                }
                let mut restrict: Vec<&Pair> = subj.pairs.iter().filter_map(|pid| mem.pairs.get(pid)).collect();
                restrict.sort_by(|a, b| cmp_content(a, b));
                (
                    queries,
                    restrict.into_iter().map(|p| p.pair_id.clone()).collect::<Vec<_>>(),
                )
            };
            let hits = store
                .search(
                    user_id,
                    &queries,
                    int_arg(args, "topK", 8, 1, 25) as usize,
                    Some(restrict),
                    None,
                    None,
                )
                .await
                .map_err(embed_error_name)?;
            let mem = store.user(user_id);
            let mem = mem.read().await;
            let idf = mem.idf();
            let q = queries.join(" ");
            Ok(sorted_previews(
                hits.iter().map(|(p, _)| preview(p, &q, Some(&idf))).collect(),
            ))
        }
        "save_memory" => {
            let prompt = ["prompt", "content", "fact", "text"]
                .iter()
                .filter_map(|k| args.get(*k))
                .find(|v| truthy(v))
                .map(py::str_value)
                .unwrap_or_default();
            let response = ["response", "summary"]
                .iter()
                .filter_map(|k| args.get(*k))
                .find(|v| truthy(v))
                .map(py::str_value)
                .unwrap_or_default();
            if py::strip(&prompt).is_empty() && py::strip(&response).is_empty() {
                return Ok(error("nothing to save"));
            }
            // Deterministic id and time (D13): the id derives from the content, and the note is filed one second after
            // the user's latest record, so it is the newest without the wall clock reaching the model.
            let ts = store.user(user_id).read().await.after_latest();
            let id = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("ditto-memory\0{prompt}\0{response}").as_bytes(),
            );
            let pair = Pair {
                pair_id: id.to_string(),
                session_id: args
                    .get("sessionId")
                    .filter(|v| truthy(v))
                    .map(py::str_value)
                    .unwrap_or_default(),
                ts,
                prompt,
                response,
                source: "agent_tool".into(),
                subjects: BTreeSet::new(),
            };
            let receipt = json!({"memory": {"id": pair.pair_id, "timestamp": fmt_ts(&pair.ts), "saved": true}});
            store.save(user_id, pair).await.map_err(embed_error_name)?;
            Ok(receipt)
        }
        other => Ok(error(format!("unknown memory tool {}", py::repr_str(other)))),
    }
}
