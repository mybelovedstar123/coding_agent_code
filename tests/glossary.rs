//! Differential test: src/glossary.rs against harness/glossary.py
//! (fixture: tests/pyref/glossary_gen.py -> tests/fixtures/glossary.json).

mod ce_support;

use ce_support::{check_step, context, digest, load, memory, s};
use ditto_agent::config::MEMORY_CONTEXT_PREFIX as PREFIX;
use ditto_agent::glossary as g;
use ditto_agent::store::UserMemory;
use indexmap::IndexMap;
use serde_json::{json, Value};

fn pairs_json(map: &IndexMap<String, String>) -> Value {
    map.iter().map(|(k, v)| json!([k, v])).collect()
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
    let items = items(mem, &c["items"]);
    let ctx = match mem {
        Some(m) => context(m, c),
        None => c["context"].as_str().map(String::from),
    };
    let mut bad = Vec::new();
    let statements: Vec<String> = items.iter().map(|(_, p)| g::statement(p, ident)).collect();
    if digest(Some(&statements.join("\0"))) != c["statements_digest"] {
        bad.push(format!("statements digest: {statements:?}"));
    }
    let defs: Vec<Value> = statements
        .iter()
        .enumerate()
        .filter_map(|(i, st)| {
            let d = g::definitions(st);
            (!d.is_empty()).then(|| json!([i, pairs_json(&d)]))
        })
        .collect();
    if Value::from(defs.clone()) != c["definitions"] {
        bad.push(format!(
            "definitions: want {} got {}",
            c["definitions"],
            Value::from(defs)
        ));
    }
    let terms = g::glossary(&items, ident);
    if pairs_json(&terms) != c["glossary"] {
        bad.push(format!("glossary: want {} got {}", c["glossary"], pairs_json(&terms)));
    }
    let glossed = Value::from(g::glossed_statements(&items, ident, &terms));
    if glossed != c["glossed_statements"] {
        bad.push(format!(
            "glossed_statements: want {} got {glossed}",
            c["glossed_statements"]
        ));
    }
    let out = g::with_glossary(ctx.clone(), PREFIX, &items, ident);
    if let Some(err) = check_step(ctx.as_deref(), out.as_deref(), &c["with_glossary"]) {
        bad.push(format!("with_glossary: {err}"));
    }
    bad.into_iter()
        .map(|b| format!("{ident} {:?}: {b}", c.get("question")))
        .collect()
}

#[test]
fn datasets() {
    let fx = load("glossary");
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
            firing += c["with_glossary"].get("digest").is_some() as usize;
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures[..failures.len().min(10)].join("\n")
    );
    assert!(n > 1000 && firing >= 60, "coverage {n} / {firing}");
}

#[test]
fn edge_cases() {
    let fx = load("glossary");
    let e = &fx["edge"];
    for t in e["texts"].as_array().expect("texts") {
        assert_eq!(
            pairs_json(&g::definitions(s(&t["text"]))),
            t["definitions"],
            "{}",
            t["text"]
        );
    }
    for t in e["statements"].as_array().expect("statements") {
        assert_eq!(g::statement(s(&t["prompt"]), s(&t["ident"])), s(&t["statement"]), "{t}");
    }
    for t in e["glossed"].as_array().expect("glossed") {
        let terms: IndexMap<String, String> = t["terms"]
            .as_array()
            .expect("terms")
            .iter()
            .map(|kv| (s(&kv[0]).into(), s(&kv[1]).into()))
            .collect();
        assert_eq!(g::glossed(s(&t["text"]), &terms), s(&t["value"]), "{t}");
    }
    let mut failures = Vec::new();
    for c in e["cases"].as_array().expect("cases") {
        failures.extend(run_case(None, c));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
