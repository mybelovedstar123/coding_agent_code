//! Shared fixture loading for the contact_evidence, glossary and current_statements differential tests
//! (generators: tests/pyref/{contact_evidence,glossary,current_statements}_gen.py, inputs: tests/pyref/ce_records.py).

#![allow(dead_code)]

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::py;
use ditto_agent::store::{fmt_ts, parse_ts, render_pair, Pair, UserMemory};
use serde_json::{json, Value};
use std::collections::BTreeSet;
