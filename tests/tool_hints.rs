//! Differential test: src/tool_hints.rs against harness/tool_hints.py (fixtures from tests/pyref/tool_hints_gen.py).

use ditto_agent::tool_hints as th;
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tool_hints.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture json")
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

type Want = fn(&str) -> bool;
type Plain = fn(&str) -> String;
type WithTools = fn(&str, &[Value]) -> String;
