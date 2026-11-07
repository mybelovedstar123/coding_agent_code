//! Differential test: src/current_statements.rs against harness/current_statements.py
//! (fixture: tests/pyref/current_statements_gen.py -> tests/fixtures/current_statements.json).

mod ce_support;

use ce_support::{check_step, context, load, memory, s, strings};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::current_statements as cs;
use ditto_agent::store::UserMemory;
use serde_json::{json, Value};
use std::collections::HashSet;
