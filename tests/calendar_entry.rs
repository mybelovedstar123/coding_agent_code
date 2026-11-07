//! The named calendar-entry trigger (change 25): a request to have a quoted item on the user's calendar fires it;
//! unnamed calendar requests, searches and schedules do not.

use ditto_agent::tool_hints as th;
use serde_json::{json, Value};

const NAMED: &[&str] = &[
    "Get the \"cray cross workstream\" review onto my calendar for Tuesday afternoon.",
    "I want the \"dark reach review\" review on my calendar Wednesday — sort that out.",
    "Make sure the \"hankyu light brief\" review is on my calendar for Thursday.",
    "Maie sure the \"uclouvain brook workstream\" review is on my caldndar for Thursday.",
    "Get the \"cray cross workstream\" review ogto my calendar for Tuesday afternoon.",
];
