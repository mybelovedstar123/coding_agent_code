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
