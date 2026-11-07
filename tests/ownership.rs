//! Differential test: src/ownership.rs against harness/ownership.py
//! (fixture: tests/pyref/ownership_gen.py -> tests/fixtures/ownership.json).

use ditto_agent::ownership as o;
use ditto_agent::store::{epoch, render_pair, Pair};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

static FIXTURE: LazyLock<Value> = LazyLock::new(|| {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ownership.json");
    let text = std::fs::read_to_string(path).expect("fixture (run tests/pyref/ownership_gen.py)");
    serde_json::from_str(&text).expect("fixture json")
});

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("string expected, got {v}"))
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().map(String::from)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

fn chain_of(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("chain")
        .iter()
        .map(|x| (s(&x[0]).to_string(), s(&x[1]).to_string()))
        .collect()
}

fn chain_json(ch: &[(String, String)]) -> Value {
    ch.iter().map(|(a, b)| json!([a, b])).collect()
}

fn sorted(set: &HashSet<String>) -> Vec<String> {
    set.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect()
}

fn report(name: &str, total: usize, bad: &[String]) {
    eprintln!("{name}: {} of {total} cases equal", total - bad.len());
    for b in bad.iter().take(10) {
        eprintln!("  MISMATCH {b}");
    }
    assert!(bad.is_empty(), "{name}: {} of {total} cases differ", bad.len());
}

#[test]
fn sequence_ratio_matches_difflib() {
    let cases = FIXTURE["sequence_ratio"].as_array().expect("cases");
    let mut bad = Vec::new();
    for c in cases {
        let (a, b) = (s(&c[0]), s(&c[1]));
        let want = c[2].as_f64().expect("ratio");
        let got = ditto_agent::difflib::sequence_ratio(a, b);
        if got.to_bits() != want.to_bits() {
            bad.push(format!("{a:?} / {b:?}: want {want} got {got}"));
        }
    }
    report("sequence_ratio", cases.len(), &bad);
}

#[test]
fn sentence_split() {
    // The fast splitter against Python's split on crafted text, and against the faithful lookbehind pattern on
    // every record.
    let cases = FIXTURE["sentences"].as_array().expect("cases");
    let mut bad = Vec::new();
    for c in cases {
        let text = s(&c[0]);
        let want = strings(&c[1]);
        if o::sentences(text) != want || o::SENT.split(text) != want {
            bad.push(format!(
                "{text:?}: want {want:?} got {:?} / {:?}",
                o::sentences(text),
                o::SENT.split(text)
            ));
        }
    }
    let mut total = cases.len();
    for ds in FIXTURE["datasets"].as_array().expect("datasets") {
        for r in ds["records"].as_array().expect("records") {
            for text in [s(&r[2]), s(&r[3])] {
                total += 1;
                if o::sentences(text) != o::SENT.split(text) {
                    bad.push(format!("{text:?}"));
                }
            }
        }
    }
    report("sentences", total, &bad);
}

#[test]
fn holder_questions() {
    let cases = FIXTURE["holder_requests"].as_array().expect("cases");
    let mut bad = Vec::new();
    for c in cases {
        let r = s(&c["request"]);
        let got = json!({
            "asks_holder": o::asks_holder(r),
            "lower_grams": sorted(&o::lower_grams(r)),
        });
        let want = json!({
            "asks_holder": c["asks_holder"],
            "lower_grams": c["lower_grams"],
        });
        if got != want {
            bad.push(format!("{r:?}: want {want} got {got}"));
        }
    }
    report("holder_requests", cases.len(), &bad);
}

#[test]
fn datasets() {
    let prefix = s(&FIXTURE["prefix"]);
    assert_eq!(prefix, ditto_agent::config::MEMORY_CONTEXT_PREFIX);
    for ds in FIXTURE["datasets"].as_array().expect("datasets") {
        let name = s(&ds["dataset"]);
        let recs = ds["records"].as_array().expect("records");
        let records: Vec<(String, String, String)> = recs
            .iter()
            .map(|r| (s(&r[0]).to_string(), s(&r[1]).to_string(), s(&r[2]).to_string()))
            .collect();
        let full: HashMap<String, Value> = recs
            .iter()
            .map(|r| {
                let pair = Pair {
                    pair_id: s(&r[0]).to_string(),
                    session_id: String::new(),
                    ts: epoch(),
                    prompt: s(&r[2]).to_string(),
                    response: s(&r[3]).to_string(),
                    source: "seed".into(),
                    subjects: BTreeSet::new(),
                };
                let rec = json!({"pairID": pair.pair_id, "timestamp": s(&r[1]), "text": render_pair(&pair)});
                (pair.pair_id.clone(), rec)
            })
            .collect();
        let idx = o::IndexV2::new(&records);
        assert_eq!(idx.pids, strings(&ds["index_order"]), "{name}: index order");
        assert_eq!(sorted(&idx.orgs), strings(&ds["orgs"]), "{name}: orgs");

        // Requests: holder tests, tokens, best_chain and its parts.
        let cases = ds["requests"].as_array().expect("requests");
        let mut bad = Vec::new();
        for c in cases {
            let r = s(&c["request"]);
            let mut got = json!({
                "request": r,
                "asks_holder": o::asks_holder(r),
                    "lower_grams": sorted(&o::lower_grams(r)),
                "tokens": o::tokens(r),
            });
            if c.get("chain").is_some() {
                let (phrase, chain) = idx.best_chain(r);
                got["phrase"] = json!(phrase);
                got["chain"] = chain_json(&chain);
                if !chain.is_empty() {
                    let phrase = phrase.unwrap_or_default();
                    got["sources"] = json!(idx.sources(&phrase));
                    got["occurs"] = json!(idx.occurs(&phrase));
                    got["candidates"] = json!(idx.candidates(&phrase));
                    got["holder_now"] = json!(o::holder_now(&chain, &idx.orgs));
                    got["first_named_holder"] = json!(o::first_named_holder(&chain));
                }
            }
            // The fixture also records asks_current_holder, removed in change 20 with the path it gated.
            let mut want = c.clone();
            if let Some(o) = want.as_object_mut() {
                o.remove("asks_current_holder");
            }
            if got != want {
                bad.push(format!("{r:?}:\n want {want}\n got  {got}"));
            }
        }
        report(&format!("{name} requests"), cases.len(), &bad);

        // Index methods over phrases.
        let cases = ds["index_cases"].as_array().expect("index_cases");
        let mut bad = Vec::new();
        for c in cases {
            let ph = s(&c["phrase"]);
            let got = json!({
                "phrase": ph,
                "candidates": idx.candidates(ph),
                "occurs": idx.occurs(ph),
                "chain": chain_json(&idx.chain(ph)),
                "sources": idx.sources(ph),
            });
            if &got != c {
                bad.push(format!("{ph:?}:\n want {c}\n got  {got}"));
            }
        }
        report(&format!("{name} index_cases"), cases.len(), &bad);

        // with_ownership.
        let cases = ds["with_ownership"].as_array().expect("with_ownership");
        let mut bad = Vec::new();
        for c in cases {
            let r = s(&c["request"]);
            let ctx = opt(&c["context"]);
            let want = if c["same"].as_bool().expect("same") {
                ctx.clone()
            } else {
                opt(&c["out"])
            };
            let got = o::with_ownership(ctx.clone(), prefix, &idx, r, |pid| full[pid].clone());
            if got != want {
                bad.push(format!("{r:?} / {ctx:?}:\n want {want:?}\n got  {got:?}"));
            }
        }
        report(&format!("{name} with_ownership"), cases.len(), &bad);

        // fuzzy_in, thread_like, is_person.
        let cases = ds["fuzzy_in"].as_array().expect("fuzzy_in");
        let mut bad = Vec::new();
        for c in cases {
            let toks = strings(&c[1]);
            let got = o::fuzzy_in(s(&c[0]), &toks, c[2].as_f64());
            if Value::Bool(got) != c[3] {
                bad.push(format!("{c}"));
            }
        }
        report(&format!("{name} fuzzy_in"), cases.len(), &bad);
        let cases = ds["thread_like"].as_array().expect("thread_like");
        let bad: Vec<String> = cases
            .iter()
            .filter(|c| Value::Bool(o::thread_like(s(&c[0]))) != c[1])
            .map(|c| c.to_string())
            .collect();
        report(&format!("{name} thread_like"), cases.len(), &bad);
        let cases = ds["is_person"].as_array().expect("is_person");
        let bad: Vec<String> = cases
            .iter()
            .filter(|c| Value::Bool(o::is_person(s(&c[0]))) != c[1])
            .map(|c| c.to_string())
            .collect();
        report(&format!("{name} is_person"), cases.len(), &bad);

        // holder_now / first_named_holder over synthetic chains.
        let cases = ds["holder_chains"].as_array().expect("holder_chains");
        let mut bad = Vec::new();
        for c in cases {
            let chain = chain_of(&c["chain"]);
            let orgs: HashSet<String> = strings(&c["orgs"]).into_iter().collect();
            let got = (o::holder_now(&chain, &orgs), o::first_named_holder(&chain));
            let want = (opt(&c["holder_now"]), opt(&c["first_named_holder"]));
            if got != want {
                bad.push(format!("{}: want {want:?} got {got:?}", c["chain"]));
            }
        }
        report(&format!("{name} holder_chains"), cases.len(), &bad);
    }
}
