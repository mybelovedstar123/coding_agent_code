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

#[test]
fn chat_tool_preference_requests_fire() {
    for text in [
        "Change my chat tools so you enable image tools.",
        "Change my chat tools so you turn off agent jobs.",
        "From now on, allow only memory tuols in our chats.",
        "Going forward, tjrn off agent jobs — lock that in.",
    ] {
        assert!(th::wants_chat_tool_prefs(text), "should fire: {text}");
    }
    for text in [
        "Is there a way to turn on dark mode?",
        "Send an agent off to build a small Flask app that serves a health check asynchronously.",
        "Make the app accent sllate ble-ish.",
    ] {
        assert!(!th::wants_chat_tool_prefs(text), "should not fire: {text}");
    }
}

#[test]
fn capability_questions_fire() {
    for text in [
        "Is there a way to set up a recurring reminder?",
        "Is thhere a way to turn on dark mode? Appreciate it.",
        "Ok, so — where do I find the optjion for tool permissions?",
        "When you get a sec, is thsre a way to set up a recurring remmimder? No rush.",
    ] {
        assert!(th::asks_capability(text), "should fire: {text}");
    }
    for text in [
        "Switch my accent to the grey shade of slatd — check the configured appearance options first.",
        "Set the chat font to Teo, or the configured option closest to that; look the list up first.",
        "Turn on dark mode.",
    ] {
        assert!(!th::asks_capability(text), "should not fire: {text}");
    }
}

#[test]
fn work_requests_fire() {
    for text in [
        "Go ahead and refactor the utils file to remove dupliacitln for me as son as you can.",
        "I want you to write a scrmit to ddepue my CSV export. Start it.",
        "Please actually srcape the latest headlines, don't just tell me how.",
    ] {
        assert!(th::wants_work_done(text), "should fire: {text}");
    }
    for text in [
        "Remember Sunny's thing, transaero side ledger? Who is actually running it now?",
        "Don't build anything — just do the arithmdtic: what's the copmund total of 1,024, 51.2, and 74?",
        "This is a one-off calculation, not a coding job: normalize 48.5, 277, and 310 to sum to 1.",
    ] {
        assert!(!th::wants_work_done(text), "should not fire: {text}");
    }
}

#[test]
fn approximate_option_requests_fire() {
    for text in [
        "Use the condensed edition of Dira Sasn for chat. Verify the exact option name against the workspace's fonts.",
        "Change the chat typefaase to Qlfa Sla OOne-ish — check which fonts this workspace has configured before setting it.",
        "Set the chat font to Teo, or the configured option closest to that; look the list up first, I may have botched it.",
        "Change the accent colour to mraoon. Spelling is approximate, so confirm it against the options this workspace lists.",
    ] {
        assert!(th::names_option_approximately(text), "should fire: {text}");
    }
    for text in [
        "Make sure my Ditto accent is velvet for me.",
        "Is there a way to turn on dark mode?",
        "Set my theme to dark.",
    ] {
        assert!(!th::names_option_approximately(text), "should not fire: {text}");
    }
}
