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
