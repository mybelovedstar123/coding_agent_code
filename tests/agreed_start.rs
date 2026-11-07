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
