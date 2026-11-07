//! Differential test: src/glossary.rs against harness/glossary.py
//! (fixture: tests/pyref/glossary_gen.py -> tests/fixtures/glossary.json).

mod ce_support;

use ce_support::{check_step, context, digest, load, memory, s};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::glossary as g;
use ditto_agent::store::UserMemory;
use indexmap::IndexMap;
use serde_json::{json, Value};

fn pairs_json(map: &IndexMap<String, String>) -> Value {
    map.iter().map(|(k, v)| json!([k, v])).collect()
}
