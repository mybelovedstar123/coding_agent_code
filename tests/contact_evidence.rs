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

/// Every public function on one case; returns the mismatches.
fn run_case(mem: &UserMemory, c: &Value) -> Vec<String> {
    let q = s(&c["question"]);
    let ctx = context(mem, c);
    let mut bad = Vec::new();
    let mut eq = |what: &str, got: Value, want: &Value| {
        if got != *want {
            bad.push(format!("{what}: want {want} got {got}"));
        }
    };
    eq("asks_for_handle", ce::asks_for_handle(q).into(), &c["asks_for_handle"]);
    eq(
        "asks_owner_contact",
        ce::asks_owner_contact(q).into(),
        &c["asks_owner_contact"],
    );
    eq(
        "person_name_forms",
        sorted(ce::person_name_forms(mem, q)).into(),
        &c["person_name_forms"],
    );
    let channel = ce::channel_records(mem, q);
    eq("channel_records", ids(&channel).into(), &c["channel_records"]);
    eq(
        "has_removal_instruction",
        ce::has_removal_instruction(&channel).into(),
        &c["has_removal_instruction"],
    );
    let texts = ce::context_texts(ctx.as_deref());
    eq(
        "context_texts.digest",
        digest(Some(&texts)),
        &c["context_texts"]["digest"],
    );
    if let Some(v) = c["context_texts"]["value"].as_str() {
        eq("context_texts", texts.clone().into(), &v.into());
    }
    let named_ctx = ce::people_named(mem, &texts);
    eq("people_named", sorted(named_ctx.clone()).into(), &c["people_named"]);
    eq(
        "people_named(question)",
        sorted(ce::people_named(mem, q)).into(),
        &c["people_named_question"],
    );
    let emails = ce::email_records(mem, &named_ctx);
    eq("email_records", ids(&emails).into(), &c["email_records"]);
    let named = ce::possessive_nicknames(mem, q);
    let named_json: Vec<Value> = named
        .iter()
        .map(|(k, v)| Value::from(vec![k.clone(), v.clone()]))
        .collect();
    eq("possessive_nicknames", named_json.into(), &c["possessive_nicknames"]);
    let identity = ce::identity_records(mem, &named);
    eq("identity_records", ids(&identity).into(), &c["identity_records"]);
    let related = if named.is_empty() {
        String::new()
    } else {
        ce::related_person_note(&named)
    };
    eq("related_person_note", related.clone().into(), &c["related_person_note"]);
    eq(
        "removed_handle_note",
        ce::removed_handle_note(mem, q).into(),
        &c["removed_handle_note"],
    );
    let steps: [(&str, Option<String>); 5] = [
        ("with_removal_note", ce::with_removal_note(ctx.clone(), PREFIX)),
        ("with_records_email", ce::with_records(ctx.clone(), PREFIX, &emails)),
        ("with_records_channel", ce::with_records(ctx.clone(), PREFIX, &channel)),
        (
            "with_records_and_note",
            ce::with_records_and_note(ctx.clone(), PREFIX, &identity, &related),
        ),
        ("pipeline", pipeline(mem, q, ctx.clone())),
    ];
    for (what, got) in steps {
        if let Some(err) = check_step(ctx.as_deref(), got.as_deref(), &c[what]) {
            bad.push(format!("{what}: {err}"));
        }
    }
    bad
}

fn fires(c: &Value) -> bool {
    let non_empty = |k: &str| c[k].as_array().is_some_and(|a| !a.is_empty());
    non_empty("channel_records")
        || c["has_removal_instruction"] == true
        || (c["asks_owner_contact"] == true && non_empty("email_records"))
        || non_empty("possessive_nicknames")
        || c["removed_handle_note"] != ""
        || c["pipeline"].get("digest").is_some()
}

fn run_cases(mem: &UserMemory, cases: &[Value], label: &str) -> (usize, usize) {
    let mut failures = Vec::new();
    for c in cases {
        for err in run_case(mem, c) {
            failures.push(format!("[{label}] {:?}: {err}", c["question"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    (cases.len(), cases.iter().filter(|c| fires(c)).count())
}

#[test]
fn datasets() {
    let fx = load("contact_evidence");
    assert_eq!(s(&fx["prefix"]), PREFIX);
    let mut total = (0, 0);
    for ds in fx["datasets"].as_array().expect("datasets") {
        let mem = memory(s(&ds["user_id"]), &ds["records"]);
        let (n, f) = run_cases(&mem, ds["cases"].as_array().expect("cases"), s(&ds["name"]));
        total = (total.0 + n, total.1 + f);
    }
    assert!(total.0 >= 1000 && total.1 >= 60, "coverage {total:?}");
}
