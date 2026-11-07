//! The named calendar-entry trigger (change 25): a request to have a quoted item on the user's calendar fires it;
//! unnamed calendar requests, searches and schedules do not.

use ditto_agent::tool_hints as th;
use serde_json::{json, Value};
