//! Shared fixture loading for the contact_evidence, glossary and current_statements differential tests
//! (generators: tests/pyref/{contact_evidence,glossary,current_statements}_gen.py, inputs: tests/pyref/ce_records.py).

#![allow(dead_code)]

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::py;
use ditto_agent::store::{fmt_ts, parse_ts, render_pair, Pair, UserMemory};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn load(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e} (run tests/pyref/{name}_gen.py)"));
    serde_json::from_str(&text).expect("fixture json")
}

pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}
