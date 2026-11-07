//! Request triggers added after the port: agreed start (24), team feedback (26), chat tool preferences (27),
//! capability questions (28), software work to do (29), approximately spelled options (31).
//! The agreed-start note needs a catalog that can list and run saved workflows; no trigger fires on look-alikes.

use ditto_agent::tool_hints as th;
use serde_json::{json, Value};

const AGREED: &[&str] = &[
    "Go ahead and start the dependency-risk review on \"lenfilm cross brief\" — per what we landed on. Go.",
    "Kick off the dependency-risk review for \"krispy bank brief\" the way we already agreed. Start now.",
    "Please begin the dependency-risk review for \"bloomberg reach review\" following what we decided together.",
    "Time to start the dependency-risk review on \"kwang reach review\" — exactly as we settled earlier. Begin.",
    "Let's start the depeency-risk review for \"polestar reach brief\" per what we lnaded on. Go.",
    "Go ahead and start the depeendeny-risk review on \"lenfilm cross brief\" — per what we lanced on. Go.",
    "Kick off the review the way we agreed",
];

const OTHER: &[&str] = &[
    "Do I already have a workflow for \"beko bank ledger\"? If there isn't one, create it under the formal name.",
    "Kick off my workflow that's set for every Monday at 08:00 — the one already saved.",
    "Start the existing workflow scheduled every Monday at 08:00 right now.",
    "Remind me — what day did we land on for the \"getty shore ledger\" handoff?",
    "What did we settle on doing about the cloud storage plan before it renews?",
    "Make a reusable workflow called trip-planner.",
];

fn catalog(names: &[&str]) -> Vec<Value> {
    names
        .iter()
        .map(|n| json!({"type": "function", "function": {"name": n, "description": "", "parameters": {}}}))
        .collect()
}

#[test]
fn agreed_start_requests_fire() {
    let tools = catalog(&["list_workflows", "run_workflow", "create_workflow"]);
    for text in AGREED {
        assert!(th::wants_agreed_start(text), "should fire: {text}");
        assert_eq!(th::agreed_start_note(text, &tools), th::AGREED_START_NOTE);
    }
}

#[test]
fn other_requests_do_not_fire() {
    for text in OTHER {
        assert!(!th::wants_agreed_start(text), "should not fire: {text}");
    }
}

#[test]
fn needs_the_workflow_tools() {
    let text = AGREED[0];
    assert_eq!(th::agreed_start_note(text, &catalog(&["run_workflow"])), "");
    assert_eq!(th::agreed_start_note(text, &catalog(&["list_workflows"])), "");
}

#[test]
fn team_feedback_requests_fire() {
    for text in [
        "Forward this note to whoever builds this: the image tool keeps timing out.",
        "Let the taem know love the new memory search.",
        "Report to the devs that love the new memory search.",
        "Send this to the Ditto team: love the new memory search.",
    ] {
        assert!(th::wants_team_feedback(text), "should fire: {text}");
    }
    for text in [
        "Use only case file c05a8f1e8702d939b. At effective step 12, follow the team of entity-0fbfe0 and consider its members.",
        "Add team offsite to my calendar.",
        "Send them the \"moto mile rollout\" numbers now.",
    ] {
        assert!(!th::wants_team_feedback(text), "should not fire: {text}");
    }
}
