//! Tool hints: a request whose wording matches what one offered tool is specifically for (when automations run and
//! how often they have run; an edit to an earlier image; arithmetic on numbers the request gives) gets a one-line note
//! naming that tool and its own description. The trigger comes from the request and the offered catalog only; the
//! note is advisory, names no value, and the model still chooses every call.
//!
//! Patterns are `event_state::PyRe`, so `\s`, `\w` and `\b` keep Python's classes.

