//! Differential test: src/event_state.rs against harness/event_state.py (fixtures from tests/pyref/event_state_gen.py).

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::event_state::{self as es, Event, PyInt};
use ditto_agent::py;
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/event_state.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture json")
}

fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("array").iter().map(|x| s(x).to_string()).collect()
}

fn evs_value(evs: &[Event]) -> Value {
    Value::Array(
        evs.iter()
            .map(|(st, op, e, f, v)| serde_json::json!([st.to_string(), op, e, f, v]))
            .collect(),
    )
}

#[test]
fn constants_match() {
    let fx = fixture();
    let c = &fx["constants"];
    assert_eq!(es::EVENT_STATE_NOTE, s(&c["EVENT_STATE_NOTE"]));
    assert_eq!(es::MIN_EVENTS as u64, c["MIN_EVENTS"].as_u64().unwrap());
    assert_eq!(es::Q, s(&c["Q"]));
}

#[test]
fn keys_and_cells_match() {
    let fx = fixture();
    for case in fx["norm_key"].as_array().unwrap() {
        assert_eq!(es::_norm_key(s(&case[0])), s(&case[1]), "{:?}", case[0]);
    }
    for case in fx["cell"].as_array().unwrap() {
        assert_eq!(es::_cell(s(&case[0])), s(&case[1]), "{:?}", case[0]);
    }
}

#[test]
fn table_readers_match() {
    let fx = fixture();
    let texts = fx["texts"].as_array().unwrap();
    let mut events = 0;
    for t in texts {
        let text = s(&t["text"]);
        for (name, f) in [
            ("from_json", es::from_json as fn(&str) -> Vec<Event>),
            ("from_csv", es::from_csv),
            ("from_markdown", es::from_markdown),
            ("from_sentences", es::from_sentences),
        ] {
            let got = f(text);
            events += got.len();
            assert_eq!(evs_value(&got), t[name], "{name} on {text:?}");
        }
    }
    assert!(
        texts.len() > 700 && events > 1000,
        "{} texts, {events} events",
        texts.len()
    );
}

#[test]
fn with_event_state_matches() {
    let fx = fixture();
    let calls = fx["calls"].as_array().unwrap();
    let mut fired = 0;
    for c in calls {
        let texts = strings(&c["texts"]);
        let ids = strings(&c["ids"]);
        let user_input = s(&c["user_input"]);
        let evs = es::events(&texts);
        assert_eq!(evs_value(&evs), c["events"], "events for {user_input:?}");
        if !evs.is_empty() {
            let step = es::requested_step(user_input, &evs);
            assert_eq!(step.to_string(), s(&c["requested_step"]), "{user_input:?}");
            assert_eq!(
                py::dumps(&es::state_at(&evs, &step), false),
                s(&c["state_at"]),
                "{user_input:?}"
            );
        }
        let context = c["context"].as_str().map(String::from);
        let got = es::with_event_state(context.clone(), MEMORY_CONTEXT_PREFIX, &ids, user_input, &texts);
        assert_eq!(
            got.as_deref(),
            c["expected"].as_str(),
            "with_event_state for {user_input:?} ({})",
            c["source"]
        );
        if got != context {
            fired += 1;
        }
    }
    assert!(calls.len() > 200 && fired >= 50, "{} calls, {fired} fired", calls.len());
}
