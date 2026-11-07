//! The confirm-a-preference trigger (change 18): requests to confirm that one of the user's own Ditto settings has
//! a named value fire it; requests to change a setting, own-appearance questions and account checks do not.

use ditto_agent::tool_hints as th;

/// Raw user inputs from the v13 datasets' wire texts (lab request logs), plus typo variants of the same requests.
const CONFIRM: &[&str] = &[
    "Check that my Ditto color mode is system please.",
    "Check that my Ditto color mode is system.",
    "Double-check that my Ditto accent is pinkish.",
    "Double-check that my Ditto color mode is system.",
    "Hey — confirm my Ditto accent is pinkish please.",
    "Hey — confirm my Ditto color mode is system — thanks.",
    "Make sure my Ditto accent is ochre when you can.",
    "Make sure my Ditto accent is velvet for me.",
    "Make sure my Ditto chat font is PT Sans Caption — thanks.",
    "Ok so, check that my Ditto chat font is Nunito for me.",
    "Ok so, check that my Ditto chat font is PT Sans Caption for me.",
    "One thing: make sure my Ditto chat font is Noto Sans Cuneiform please.",
    "Maie sure my Ditto accent is velvet for me.",
    "Moke sure my Ditto accent is ochre when you can.",
    "Ok so, check that my Ditto chat flnt is PT Sans Caption for me.",
];

const OTHER: &[&str] = &[
    "Change the accent colour to mraoon. Spelling is approximate, so confirm it against the options this workspace lists.",
    "Change the chat typeface to Alla Slab One-ish — check which fonts this workspace has configured before setting it.",
    "Set my accent to linrn (or whatever the closest configured option is called); check the appearance list before \
     you apply it.",
    "Make the app accent sllate ble-ish. I may have mangled the spelling, so check which accent options this workspace \
     actually has.",
    "Have Ditto use my usual velvit-ish accent — the personal app preference, not one of the client brand colors.",
    "Switch Ditto's accent color to something like ligth salmon — look up the configured appearance options first.",
    "If you configured my Ditto appearance now, which mode would you set?",
    "Suppose you were configuring my Ditto look today — what accent should you apply?",
    "Checking one of my accounts. What is currently owed on Baxter Griffin's account? Answer in GBP.",
    "Run the Tiru checksum routine through Code Mode and tell me precisely what it outputs.",
    "Check the agent jobs I dispatched: what's the precise item count the finished Filisit rebuild job reports?",
];

#[test]
fn confirm_requests_fire() {
    for text in CONFIRM {
        assert!(th::asks_to_confirm_preference(text), "should fire: {text}");
        assert_eq!(th::confirm_preference_note(text), th::CONFIRM_PREFERENCE_NOTE);
    }
}

#[test]
fn other_requests_do_not_fire() {
    for text in OTHER {
        assert!(!th::asks_to_confirm_preference(text), "should not fire: {text}");
        assert_eq!(th::confirm_preference_note(text), "");
    }
}

#[test]
fn own_preference_questions_are_unchanged() {
    // The shared setting test was factored out of asks_own_preference; its answers stay the same.
    assert!(th::asks_own_preference(
        "If you configured my Ditto appearance now, which mode would you set?"
    ));
    assert!(th::asks_own_preference(
        "Suppose you were configuring my Ditto look today — what accent should you apply?"
    ));
    assert!(!th::asks_own_preference("Make sure my Ditto accent is velvet for me."));
}
