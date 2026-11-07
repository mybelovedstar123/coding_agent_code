//! Differential test: src/current_statements.rs against harness/current_statements.py
//! (fixture: tests/pyref/current_statements_gen.py -> tests/fixtures/current_statements.json).

mod ce_support;

use ce_support::{check_step, context, load, memory, s, strings};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::current_statements as cs;
use ditto_agent::store::UserMemory;
use serde_json::{json, Value};
use std::collections::HashSet;

fn sorted_words(set: HashSet<String>) -> Value {
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v.into()
}

fn items(mem: Option<&UserMemory>, v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("items")
        .iter()
        .map(|it| match it {
            Value::String(pid) => (pid.clone(), mem.expect("memory").pairs[pid].prompt.clone()),
            pair => (s(&pair[0]).to_string(), s(&pair[1]).to_string()),
        })
        .collect()
}

fn run_case(mem: Option<&UserMemory>, c: &Value) -> Vec<String> {
    let ident = s(&c["ident"]);
    let request = s(&c["request"]);
    let items = items(mem, &c["items"]);
    let ctx = match mem {
        Some(m) => context(m, c),
        None => c["context"].as_str().map(String::from),
    };
    let mut bad = Vec::new();
    if sorted_words(cs::words(request)) != c["words"] {
        bad.push(format!(
            "words(request): want {} got {}",
            c["words"],
            sorted_words(cs::words(request))
        ));
    }
    if c["short"] == true {
        let notes: Vec<Value> = cs::notes_of(&items, ident)
            .into_iter()
            .map(|n| {
                json!({
                    "text": n.text,
                    "records": n.records,
                    "is_change": cs::is_change(&n.text),
                    "is_aside": cs::is_aside(&n.text),
                    "list_items": cs::list_items(&n.text),
                    "words": sorted_words(cs::words(&n.text)),
                })
            })
            .collect();
        let want = c["notes"].as_array().expect("notes");
        if notes.len() != want.len() {
            bad.push(format!("notes: want {} got {}", want.len(), notes.len()));
        }
        for (got, want) in notes.iter().zip(want) {
            if got != want {
                bad.push(format!("note: want {want} got {got}"));
            }
        }
        let found = Value::from(cs::current_statements(&items, ident, request));
        if found != c["current_statements"] {
            bad.push(format!(
                "current_statements: want {} got {found}",
                c["current_statements"]
            ));
        }
    }
    let out = cs::with_current_statements(ctx.clone(), PREFIX, &items, ident, request);
    if let Some(err) = check_step(ctx.as_deref(), out.as_deref(), &c["with_current_statements"]) {
        bad.push(format!("with_current_statements: {err}"));
    }
    bad.into_iter()
        .map(|b| format!("{ident} {:?}: {b}", c.get("category")))
        .collect()
}

#[test]
fn datasets() {
    let fx = load("current_statements");
    let (mut n, mut firing) = (0, 0);
    let mut failures = Vec::new();
    for ds in fx["datasets"].as_array().expect("datasets") {
        let mem = memory(s(&ds["user_id"]), &ds["records"]);
        for c in ds["requests"]
            .as_array()
            .expect("requests")
            .iter()
            .chain(ds["sweep"].as_array().expect("sweep"))
        {
            failures.extend(run_case(Some(&mem), c));
            n += 1;
            firing += c["with_current_statements"].get("digest").is_some() as usize;
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures[..failures.len().min(10)].join("\n")
    );
    assert!(n > 500 && firing >= 200, "coverage {n} / {firing}");
}
