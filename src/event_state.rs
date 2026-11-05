//! Event histories in stored records -> the state they describe at an effective step.
//!
//! Records of an event history state their own rules (events apply by effective step, not document order; an
//! assignment replaces a field's value; add/remove change only the named member of a set; state persists until
//! changed). The events come as tables (JSON lists of objects, CSV, markdown tables with entity / field / value /
//! effective step / operation columns) or as sentences that quote the entity, field and value and name the step. This
//! module reads the events, applies those rules up to a step, and returns every entity's fields at that step, so the
//! deciding model sees the state the records describe next to the records themselves.
//!
//! Python semantics carried here (private helpers, see the report): `json.loads` (NaN/Infinity, arbitrary-precision
//! ints, the 4300-digit int limit), `int()` of a str/float, `str.splitlines()`, the `csv` module's reader state machine
//! with `DictReader`, and Python's `\s`/`\w`/`\b` classes in patterns (`py_pattern`).

use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

pub const Q: &str = r#""([^"]+)""#;
pub static STEP_RE: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?i)\bstep\s+(\d+)\b"));
pub static SENT_SPLIT: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?<=[.;])\s+(?=[A-Z\[])|\n+"));
pub const MIN_EVENTS: usize = 10;
pub const EVENT_STATE_NOTE: &str = "event_state: a host-side reading of the event history in the records for the \
identifier in the request, replayed up to effective step {step} under the rules those records state (events by \
effective step, not document order; an assignment replaces the field's value; add/remove change only the named member \
of a set; state persists until changed), with every entity's fields at that step and sets as lists. It is an \
unverified candidate, not an answer, and can be wrong: check what you rely on against the records themselves, and \
where they differ, the records decide.";
