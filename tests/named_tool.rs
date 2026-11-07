//! The named-tool note: a request that names an offered tool by its distinctive first word gets the note, which says
//! to call the tool rather than look elsewhere or ask the user first (change 32).

use ditto_agent::named_tool::named_tool_note;
use serde_json::{json, Value};

fn catalog(names: &[&str]) -> Vec<Value> {
    names
        .iter()
        .map(|n| json!({"type": "function", "function": {"name": n, "description": "", "parameters": {}}}))
        .collect()
}
