//! Differential test: src/tool_hints.rs against harness/tool_hints.py (fixtures from tests/pyref/tool_hints_gen.py).

use ditto_agent::tool_hints as th;
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tool_hints.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture json")
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

type Want = fn(&str) -> bool;
type Plain = fn(&str) -> String;
type WithTools = fn(&str, &[Value]) -> String;

const WANTS: &[(&str, Want)] = &[
    ("wants_schedules", th::wants_schedules),
    ("wants_image_edit", th::wants_image_edit),
    ("wants_arithmetic", th::wants_arithmetic),
    ("wants_background_job", th::wants_background_job),
    ("wants_calendar_entry", th::wants_calendar_entry),
    ("wants_sandbox_run", th::wants_sandbox_run),
    ("wants_job_figure", th::wants_job_figure),
    ("wants_note_fix", th::wants_note_fix),
    ("wants_research_send", th::wants_research_send),
    ("wants_usual_effort", th::wants_usual_effort),
    ("asks_own_preference", th::asks_own_preference),
    ("wants_send_on_record", th::wants_send_on_record),
    ("wants_exact_value", th::wants_exact_value),
];
const PLAIN: &[(&str, Plain)] = &[
    ("exact_value_note", th::exact_value_note),
    ("preference_note", th::preference_note),
];
const WITH_TOOLS: &[(&str, WithTools)] = &[
    ("research_send_note", th::research_send_note),
    ("effort_note", th::effort_note),
    ("tool_hint_note", th::tool_hint_note),
    ("send_on_record_note", th::send_on_record_note),
];

/// Tool-hint rules added after the port (change 26 on), in order after the reference's rules. Each fires only where
/// the reference produced no tool-hint note.
const ADDED_RULES: &[&str] = &[
    "file_feedback_for_team",
    "set_chat_tool_preferences",
    "discover_capabilities",
    "execute_agent_job",
];

/// Requests the usual-effort trigger takes since change 33 (a typo in "reasoning", a "Quick one:" opener) where the
/// reference did not; on these, the trigger and the effort note may newly fire.
const EFFORT_WIDENED: &[&str] = &[
    "For planning context c02f07f38c3117d82: Quick one: sort out your reewoning depth.",
    "For planning context caae4db14610d268b: When you get a sec, dial in my reasoning ecfort the way I lioe it.",
    "For planning context c7f0276a3ea7b669d: Sort out your rasojing depth for me.",
];

fn effort_widened(text: &str, name: &str) -> bool {
    matches!(name, "wants_usual_effort" | "effort_note") && EFFORT_WIDENED.contains(&text)
}

/// A note one of the added rules wrote.
fn from_added_rule(note: &str) -> bool {
    let prefix = &th::TOOL_HINT_NOTE[..th::TOOL_HINT_NOTE.find("{tool}").expect("placeholder")];
    ADDED_RULES.iter().any(|t| note.starts_with(&format!("{prefix}{t} ")))
}

#[test]
fn constants_and_rules_match() {
    let fx = fixture();
    let c = &fx["constants"];
    for (name, value) in [
        ("TOOL_HINT_NOTE", th::TOOL_HINT_NOTE),
        ("RESEARCH_SEND_NOTE", th::RESEARCH_SEND_NOTE),
        ("EFFORT_NOTE", th::EFFORT_NOTE),
        ("PREFERENCE_NOTE", th::PREFERENCE_NOTE),
        ("SEND_ON_RECORD_NOTE", th::SEND_ON_RECORD_NOTE),
        ("EXACT_VALUE_NOTE", th::EXACT_VALUE_NOTE),
    ] {
        assert_eq!(value, s(&c[name]), "{name}");
    }
    let rules: Vec<(String, String)> = fx["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (s(&r[0]).to_string(), s(&r[1]).to_string()))
        .collect();
    assert_eq!(rules.len() + ADDED_RULES.len(), th::RULES.len());
    for ((tool, _), added) in th::RULES[rules.len()..].iter().zip(ADDED_RULES) {
        assert_eq!(tool, added);
    }
    for ((tool, test), (want_tool, want_test)) in th::RULES.iter().zip(&rules) {
        assert_eq!(tool, want_tool);
        let f = WANTS.iter().find(|(n, _)| n == want_test).expect("known test").1;
        assert!(std::ptr::fn_addr_eq(*test, f), "{tool}: {want_test}");
    }
    let extra = fx["rule_extra"].as_object().unwrap();
    assert_eq!(
        extra.len(),
        th::RULE_EXTRA.iter().filter(|(t, _)| !ADDED_RULES.contains(t)).count()
    );
    for (tool, text) in th::RULE_EXTRA.iter().filter(|(t, _)| !ADDED_RULES.contains(t)) {
        assert_eq!(*text, s(&extra[*tool]), "{tool}");
    }
}

#[test]
fn triggers_match() {
    let fx = fixture();
    let reqs = fx["requests"].as_array().unwrap();
    let mut differ = Vec::new();
    for r in reqs {
        let text = s(&r["text"]);
        for (name, f) in WANTS {
            if effort_widened(text, name) && f(text) && !r[*name].as_bool().unwrap() {
                continue;
            }
            if f(text) != r[*name].as_bool().unwrap() {
                differ.push(format!("{name}({text:?}) = {}", f(text)));
            }
        }
        for (name, f) in PLAIN {
            assert_eq!(f(text), s(&r[*name]), "{name}({text:?})");
        }
        let words: Vec<String> = r["words"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| s(w).to_string())
            .collect();
        assert_eq!(th::_words(text), words, "_words({text:?})");
        let nums: Vec<String> = r["numbers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| s(w).to_string())
            .collect();
        assert_eq!(th::NUMBER_RE.findall(text), nums, "NUMBER_RE({text:?})");
    }
    assert!(
        differ.is_empty(),
        "{} triggers differ:\n{}",
        differ.len(),
        differ.join("\n")
    );
    assert!(reqs.len() > 1000);
}

#[test]
fn notes_match() {
    let fx = fixture();
    let reqs = fx["requests"].as_array().unwrap();
    let cats: Vec<Vec<Value>> = fx["catalogs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["tool_defs"].as_array().unwrap().clone())
        .collect();
    let mut fired = 0;
    for row in fx["notes"].as_array().unwrap() {
        let text = s(&reqs[row["req"].as_u64().unwrap() as usize]["text"]);
        let defs = &cats[row["cat"].as_u64().unwrap() as usize];
        for (name, f) in WITH_TOOLS {
            let got = f(text, defs);
            fired += usize::from(!got.is_empty());
            let want = s(&row[*name]);
            if want.is_empty() && (*name == "tool_hint_note" && from_added_rule(&got) || effort_widened(text, name)) {
                continue;
            }
            assert_eq!(got, want, "{name}({text:?}) on catalog {}", row["cat"]);
        }
    }
    assert!(fired > 600, "{fired} notes fired");
}
