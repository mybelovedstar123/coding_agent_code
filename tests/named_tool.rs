//! The named-tool note: a request that names an offered tool by its distinctive first word gets the note, which says
//! to call the tool rather than look elsewhere or ask the user first (change 32).

use ditto_agent::named_tool::named_tool_note;
use serde_json::{json, Value};

fn catalog(names: &[&str]) -> Vec<Value> {
    names
        .iter()
        .map(|n| json!({"type": "function", "function": {"name": n, "description": "", "parameters": {}}}))
        .collect()
}

#[test]
fn a_named_tool_gets_the_note() {
    let tools = catalog(&["search_web", "lamuket_focus_timer", "set_theme"]);
    let note = named_tool_note(
        "Begin a Lamuket focus session for the Flybe lattice and report the exact value in the confirmation.",
        &tools,
    );
    assert!(note.contains("lamuket_focus_timer"), "{note}");
    assert!(note.contains("do not ask the user for them first"), "{note}");
}

#[test]
fn no_named_tool_no_note() {
    let tools = catalog(&["search_web", "lamuket_focus_timer"]);
    assert_eq!(named_tool_note("Start a focus timer for my work please.", &tools), "");
    // The brand as the first word of a sentence is not a name.
    assert_eq!(named_tool_note("Lamuket.", &catalog(&["search_web"])), "");
}
