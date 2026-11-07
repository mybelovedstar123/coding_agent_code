//! Differential test: src/record_order.rs against harness/record_order.py
//! (fixture: tests/pyref/record_order_gen.py -> tests/fixtures/record_order.json).

use chrono::{DateTime, Utc};
use ditto_agent::record_order as ro;
use serde_json::{json, Value};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}

fn ids(sets: &[Vec<ro::Copy>]) -> Value {
    sets.iter()
        .map(|set| set.iter().map(|(pid, _, _)| pid.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .into()
}

#[test]
fn record_order_matches_python() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/record_order.json");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture (run tests/pyref/record_order_gen.py)"))
            .expect("fixture json");
    let (mut total, mut notes) = (0, 0);
    for ds in fixture["datasets"].as_array().expect("datasets") {
        let name = s(&ds["dataset"]);
        let table = &ds["records"];
        let cases = ds["cases"].as_array().expect("cases");
        let mut bad = Vec::new();
        for c in cases {
            let ident = s(&c["ident"]);
            let items: Vec<ro::Record> = c["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|pid| {
                    let r = &table[s(pid)];
                    let ts = DateTime::parse_from_rfc3339(s(&r[1]))
                        .expect("isoformat")
                        .with_timezone(&Utc);
                    (s(pid).to_string(), s(&r[0]).to_string(), ts, s(&r[2]).to_string())
                })
                .collect();
            let statements: Vec<String> = items.iter().map(|it| ro::statement(&it.3, ident)).collect();
            let words: Vec<Vec<String>> = statements
                .iter()
                .map(|st| ro::words(st).into_iter().collect())
                .collect();
            let got = json!({
                "ident": ident,
                "items": c["items"],
                "statements": statements,
                "words": words,
                "copy_sets": ids(&ro::copy_sets(&items, ident)),
                "reordered_sets": ids(&ro::reordered_sets(&items, ident)),
                "note": ro::copies_note(&items, ident),
            });
            if &got != c {
                bad.push(format!("{ident}:\n want {c}\n got  {got}"));
            }
            if !ro::copies_note(&items, ident).is_empty() {
                notes += 1;
            }
        }
        eprintln!("{name}: {} of {} cases equal", cases.len() - bad.len(), cases.len());
        for b in bad.iter().take(5) {
            eprintln!("  MISMATCH {b}");
        }
        assert!(bad.is_empty(), "{name}: {} mismatches", bad.len());
        total += cases.len();
    }
    eprintln!("copies_note: {total} cases, {notes} with a note");
}
