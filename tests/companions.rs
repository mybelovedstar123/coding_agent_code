//! Differential test: src/companions.rs against harness/companions.py
//! (fixture: tests/pyref/companions_gen.py -> tests/fixtures/companions.json).

use chrono::{DateTime, Utc};
use ditto_agent::companions as c;
use ditto_agent::store::{cmp_content, Pair};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

fn ts(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .expect("isoformat timestamp")
        .with_timezone(&Utc)
}

#[test]
fn companions_match_python() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/companions.json");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture (run tests/pyref/companions_gen.py)"))
            .expect("fixture json");
    let mut total = 0;
    let mut fired = 0;
    for ds in fixture["datasets"].as_array().expect("datasets") {
        let name = s(&ds["dataset"]);
        let mut pairs: IndexMap<String, Pair> = IndexMap::new();
        for r in ds["records"].as_array().expect("records") {
            let pair = Pair {
                pair_id: s(&r[0]).to_string(),
                session_id: String::new(),
                ts: ts(s(&r[1])),
                prompt: s(&r[2]).to_string(),
                response: s(&r[3]).to_string(),
                source: "seed".into(),
                subjects: BTreeSet::new(),
            };
            pairs.insert(pair.pair_id.clone(), pair);
        }
        // The agent's caller: build_index(sorted(mem.pairs.values(), key=content_key)).
        let mut ordered: Vec<&Pair> = pairs.values().collect();
        ordered.sort_by(|a, b| cmp_content(a, b));
        let order: Vec<String> = ordered.iter().map(|p| p.pair_id.clone()).collect();
        assert_eq!(order, strings(&ds["sorted"]), "{name}: content-key order");
        let index = c::build_index(&ordered);
        let joint: Value = index
            .joint
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect::<serde_json::Map<_, _>>()
            .into();
        assert_eq!(joint, ds["joint"], "{name}: joint");
        assert_eq!(
            json!(index.phrase_records.len()),
            ds["phrase_records_len"],
            "{name}: phrase count"
        );
        assert_eq!(
            json!(index.phrase_records.values().sum::<usize>()),
            ds["phrase_records_total"],
            "{name}: phrase total"
        );
        let mut view = pairs.clone();
        if let Some(drop) = ds.get("drop") {
            for pid in strings(drop) {
                view.shift_remove(&pid);
            }
        }

        let mut bad = Vec::new();
        let cases = ds["cases"].as_array().expect("cases");
        for case in cases {
            let request = s(&case["request"]);
            let anchors: Vec<&Pair> = strings(&case["anchors"]).iter().map(|pid| &pairs[pid]).collect();
            let exclude: HashSet<String> = strings(&case["exclude"]).into_iter().collect();
            let out: Vec<String> = c::companion_records(&index, &view, request, &anchors, &exclude)
                .into_iter()
                .map(|p| p.pair_id.clone())
                .collect();
            if json!(out) != case["out"] {
                bad.push(format!(
                    "{request:?} anchors {}: want {} got {out:?}",
                    case["anchors"], case["out"]
                ));
            }
            if let Some(want) = case.get("phrases") {
                let got: BTreeSet<(String, String)> = c::phrases(request).into_iter().collect();
                let got: Vec<Value> = got.iter().map(|(a, b)| json!([a, b])).collect();
                if json!(got) != *want {
                    bad.push(format!("{request:?} phrases: want {want} got {got:?}"));
                }
                let counts: BTreeSet<(String, String, usize)> = c::phrases(request)
                    .into_iter()
                    .filter_map(|ph| index.phrase_records.get(&ph).map(|n| (ph.0.clone(), ph.1.clone(), *n)))
                    .collect();
                let counts: Vec<Value> = counts.iter().map(|(a, b, n)| json!([[a, b], n])).collect();
                if json!(counts) != case["phrase_counts"] {
                    bad.push(format!(
                        "{request:?} phrase counts: want {} got {counts:?}",
                        case["phrase_counts"]
                    ));
                }
            }
            if !out.is_empty() {
                fired += 1;
            }
        }
        for pw in ds["proper_words"].as_array().expect("proper_words") {
            let got: Vec<String> = c::proper_words(s(&pw[0])).into_iter().collect();
            if json!(got) != pw[1] {
                bad.push(format!("proper_words {:?}: want {} got {got:?}", s(&pw[0]), pw[1]));
            }
        }
        eprintln!(
            "{name}: {} of {} cases equal",
            cases.len() - bad.len().min(cases.len()),
            cases.len()
        );
        for b in bad.iter().take(10) {
            eprintln!("  MISMATCH {b}");
        }
        assert!(bad.is_empty(), "{name}: {} mismatches", bad.len());
        total += cases.len();
    }
    eprintln!("companion_records: {total} cases, {fired} return records");
}
