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

/// The contact add-ons in agent._memory_context order.
fn pipeline(mem: &UserMemory, q: &str, mut context: Option<String>) -> Option<String> {
    if ce::has_removal_instruction(&ce::channel_records(mem, q)) {
        context = ce::with_removal_note(context, PREFIX);
    }
    if ce::asks_owner_contact(q) {
        let forms = ce::people_named(mem, &ce::context_texts(context.as_deref()));
        let records = if forms.is_empty() {
            Vec::new()
        } else {
            ce::email_records(mem, &forms)
        };
        if !records.is_empty() {
            context = ce::with_records(context, PREFIX, &records);
        }
    }
    let named = ce::possessive_nicknames(mem, q);
    let records = if named.is_empty() {
        Vec::new()
    } else {
        ce::identity_records(mem, &named)
    };
    if !records.is_empty() {
        context = ce::with_records_and_note(context, PREFIX, &records, &ce::related_person_note(&named));
    }
    let note = ce::removed_handle_note(mem, q);
    if !note.is_empty() {
        context = ce::with_records_and_note(context, PREFIX, &[], &note);
    }
    context
}

fn sorted(set: BTreeSet<String>) -> Vec<String> {
    set.into_iter().collect()
}

fn named_map(v: &Value) -> IndexMap<String, String> {
    v.as_array()
        .expect("pairs")
        .iter()
        .map(|kv| (s(&kv[0]).to_string(), s(&kv[1]).to_string()))
        .collect()
}
