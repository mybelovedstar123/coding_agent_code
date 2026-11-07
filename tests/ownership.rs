//! Differential test: src/ownership.rs against harness/ownership.py
//! (fixture: tests/pyref/ownership_gen.py -> tests/fixtures/ownership.json).

use ditto_agent::ownership as o;
use ditto_agent::store::{epoch, render_pair, Pair};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

static FIXTURE: LazyLock<Value> = LazyLock::new(|| {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ownership.json");
    let text = std::fs::read_to_string(path).expect("fixture (run tests/pyref/ownership_gen.py)");
    serde_json::from_str(&text).expect("fixture json")
});

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().map(String::from)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

fn chain_of(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("chain")
        .iter()
        .map(|x| (s(&x[0]).to_string(), s(&x[1]).to_string()))
        .collect()
}

fn chain_json(ch: &[(String, String)]) -> Value {
    ch.iter().map(|(a, b)| json!([a, b])).collect()
}

fn sorted(set: &HashSet<String>) -> Vec<String> {
    set.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect()
}
