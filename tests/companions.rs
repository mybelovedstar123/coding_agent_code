//! Differential test: src/companions.rs against harness/companions.py
//! (fixture: tests/pyref/companions_gen.py -> tests/fixtures/companions.json).

use chrono::{DateTime, Utc};
use ditto_agent::companions as c;
use ditto_agent::store::{cmp_content, Pair};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}
