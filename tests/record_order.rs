//! Differential test: src/record_order.rs against harness/record_order.py
//! (fixture: tests/pyref/record_order_gen.py -> tests/fixtures/record_order.json).

use chrono::{DateTime, Utc};
use ditto_agent::record_order as ro;
use serde_json::{json, Value};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}
