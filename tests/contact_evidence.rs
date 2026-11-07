//! Differential test: src/contact_evidence.rs against harness/contact_evidence.py
//! (fixture: tests/pyref/contact_evidence_gen.py -> tests/fixtures/contact_evidence.json).

mod ce_support;

use ce_support::{check_step, context, digest, ids, load, memory, s, strings};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::contact_evidence as ce;
use ditto_agent::store::UserMemory;
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::BTreeSet;
