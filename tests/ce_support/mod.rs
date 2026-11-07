//! Shared fixture loading for the contact_evidence, glossary and current_statements differential tests
//! (generators: tests/pyref/{contact_evidence,glossary,current_statements}_gen.py, inputs: tests/pyref/ce_records.py).

#![allow(dead_code)]

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::py;
use ditto_agent::store::{fmt_ts, parse_ts, render_pair, Pair, UserMemory};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn load(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e} (run tests/pyref/{name}_gen.py)"));
    serde_json::from_str(&text).expect("fixture json")
}

pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

/// A UserMemory holding the fixture records in upsert order (dummy vectors), with each timestamp checked against
/// the Python `fmt_ts(parse_ts(...))`.
pub fn memory(user_id: &str, records: &Value) -> UserMemory {
    let mut mem = UserMemory::new(user_id);
    let mut pairs = Vec::new();
    for r in records.as_array().expect("records") {
        let ts = parse_ts(r.get("timestamp"));
        assert_eq!(fmt_ts(&ts), s(&r["fmt_ts"]), "timestamp {:?}", r["timestamp"]);
        pairs.push(Pair {
            pair_id: s(&r["pair_id"]).into(),
            session_id: s(&r["session_id"]).into(),
            ts,
            prompt: s(&r["prompt"]).into(),
            response: s(&r["response"]).into(),
            source: "seed".into(),
            subjects: BTreeSet::new(),
        });
    }
    let vecs = vec![vec![0.0f32; 768]; pairs.len()];
    mem.upsert_pairs(pairs, vecs);
    mem
}

pub fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub fn digest(text: Option<&str>) -> Value {
    match text {
        None => Value::Null,
        Some(t) => json!({"len": py::len(t), "fnv": fnv(t)}),
    }
}

/// The case's context: given directly, or rebuilt from its spec as the generator built it (checked by digest).
pub fn context(mem: &UserMemory, case: &Value) -> Option<String> {
    let ctx = match &case["context_spec"] {
        Value::Null => case["context"].as_str().map(String::from),
        spec => {
            let items: Vec<Value> = spec["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|it| {
                    let p = &mem.pairs[s(&it[0])];
                    let text = render_pair(p);
                    let text = match it[1].as_u64() {
                        Some(limit) => py::head(&text, limit as usize).to_string(),
                        None => text,
                    };
                    json!({"pairID": p.pair_id, "timestamp": fmt_ts(&p.ts), "text": text})
                })
                .collect();
            let payload = json!({"note": spec["note"], "memories": items});
            Some(format!("{MEMORY_CONTEXT_PREFIX}\n{}", py::dumps(&payload, false)))
        }
    };
    if case.get("context_digest").is_some() {
        assert_eq!(
            digest(ctx.as_deref()),
            case["context_digest"],
            "context rebuild for {:?}",
            case["question"]
        );
    }
    ctx
}

/// Checks a context step against its compact expectation ({"same"}, or {"digest", "value"?}); returns a mismatch
/// description.
pub fn check_step(before: Option<&str>, after: Option<&str>, want: &Value) -> Option<String> {
    if want.get("same").is_some() {
        return (after != before).then(|| format!("expected unchanged, got {after:?}"));
    }
    if let Some(v) = want.get("value") {
        if v.as_str() != after {
            return Some(format!("value differs:\n want {}\n got  {after:?}", v));
        }
    }
    (digest(after) != want["digest"]).then(|| format!("digest differs: want {} got {after:?}", want["digest"]))
}
