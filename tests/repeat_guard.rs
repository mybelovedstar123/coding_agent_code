//! The identical-call rule (change 20): a call is identified by its tool and its arguments with keys sorted, and a
//! request that asks for something to be done again lifts the rule.

use ditto_agent::agent::{asks_to_repeat, call_key, repeat_exempt};
use serde_json::{json, Map, Value};
