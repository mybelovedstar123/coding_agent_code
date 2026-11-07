//! Differential test: src/ownership.rs against harness/ownership.py
//! (fixture: tests/pyref/ownership_gen.py -> tests/fixtures/ownership.json).

use ditto_agent::ownership as o;
use ditto_agent::store::{epoch, render_pair, Pair};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;
