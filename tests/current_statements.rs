//! Differential test: src/current_statements.rs against harness/current_statements.py
//! (fixture: tests/pyref/current_statements_gen.py -> tests/fixtures/current_statements.json).

mod ce_support;

use ce_support::{check_step, context, load, memory, s, strings};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::current_statements as cs;
use ditto_agent::store::UserMemory;
use serde_json::{json, Value};
use std::collections::HashSet;

fn sorted_words(set: HashSet<String>) -> Value {
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v.into()
}

fn items(mem: Option<&UserMemory>, v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("items")
        .iter()
        .map(|it| match it {
            Value::String(pid) => (pid.clone(), mem.expect("memory").pairs[pid].prompt.clone()),
            pair => (s(&pair[0]).to_string(), s(&pair[1]).to_string()),
        })
        .collect()
}
