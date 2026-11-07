//! The identical-call rule (change 20): a call is identified by its tool and its arguments with keys sorted, and a
//! request that asks for something to be done again lifts the rule.

use ditto_agent::agent::{asks_to_repeat, call_key, repeat_exempt};
use serde_json::{json, Map, Value};

fn args(v: Value) -> Map<String, Value> {
    v.as_object().expect("object").clone()
}

#[test]
fn same_tool_and_arguments_is_the_same_call() {
    let a = call_key("set_theme", &args(json!({"theme": "dark", "scope": {"b": 1, "a": 2}})));
    let b = call_key("set_theme", &args(json!({"scope": {"a": 2, "b": 1}, "theme": "dark"})));
    assert_eq!(a, b, "key order must not matter");
}

#[test]
fn different_arguments_or_tool_are_different_calls() {
    let base = call_key("search_web", &args(json!({"query": "ferry times"})));
    assert_ne!(base, call_key("search_web", &args(json!({"query": "ferry prices"}))));
    assert_ne!(base, call_key("search_news", &args(json!({"query": "ferry times"}))));
    assert_ne!(
        base,
        call_key("search_web", &args(json!({"query": "ferry times", "limit": 5})))
    );
}
