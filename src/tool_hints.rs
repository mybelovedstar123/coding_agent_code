//! Tool hints: a request whose wording matches what one offered tool is specifically for (when automations run and
//! how often they have run; an edit to an earlier image; arithmetic on numbers the request gives) gets a one-line note
//! naming that tool and its own description. The trigger comes from the request and the offered catalog only; the
//! note is advisory, names no value, and the model still chooses every call.
//!
//! Patterns are `event_state::PyRe`, so `\s`, `\w` and `\b` keep Python's classes.

use crate::difflib::sequence_ratio;
use crate::event_state::PyRe;
use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::Value;
use std::sync::LazyLock;

pub const TOOL_HINT_NOTE: &str = "For this request, the offered tool {tool} is the one made for it ({desc}). Call it \
once with what the request gives and answer from its result.";
pub static NUMBER_RE: LazyLock<PyRe> = LazyLock::new(|| re(r"(?<![\w.])-?\d+(?:[.,]\d+)?(?![\w])"));
