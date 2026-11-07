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
