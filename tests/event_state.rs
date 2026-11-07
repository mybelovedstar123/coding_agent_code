//! Differential test: src/event_state.rs against harness/event_state.py (fixtures from tests/pyref/event_state_gen.py).

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::event_state::{self as es, Event, PyInt};
use ditto_agent::py;
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/event_state.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture json")
}

fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

fn evs_value(evs: &[Event]) -> Value {
    Value::Array(
        evs.iter()
            .map(|(st, op, e, f, v)| serde_json::json!([st.to_string(), op, e, f, v]))
            .collect(),
    )
}
