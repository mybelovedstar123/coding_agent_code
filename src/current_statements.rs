//! Change statements in a case file and the earlier statements they supersede.
//!
//! A user's notes about one thing are often filed several times (copies, typos), and a later note says that something
//! changed ("Update — …", "X has taken over …", "Rescheduled: … is now … instead", "Revised list — … is off it, … is on
//! it", "Changed my mind …"). Copies are refiled, so filing dates do not show which note came last; the change note
//! itself says it replaces the earlier value. This module groups the notes of the request's identifier records (copies
//! by text similarity), finds the notes that state a change on the request's topic, and lists each with the earlier
//! plain notes of that topic it supersedes, so the deciding model sees which statement is current.
//!
//! Port of `harness/current_statements.py`.

use crate::difflib::sequence_ratio;
use crate::py;
use crate::pyre::{escape, Re};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub const CHANGE_CUES: &[&str] = &[
    "change of plan",
    "update",
    "updated",
    "revised",
    "changed hands",
    "has taken over",
    "took over",
    "taken over",
    "from now on",
    "going forward",
    "replacing",
    "replaces",
    "replaced",
    "instead",
    "scrap the earlier",
    "final call",
    "ignore what i said",
    "gone from what it was",
    "moves from",
    "moved from",
    "no longer",
    "after all",
    "rebooked",
    "new date",
    "now on the",
    "now, not",
    "swapped",
    "that replaces",
    "is now",
    "now cancelled",
    "now confirmed",
    "now postponed",
    "change:",
    "not the earlier",
    "changed my mind",
    "changed the",
    "as of today",
    "dropped",
    "is off it",
    "is on it",
    "has changed",
    "rescheduled",
    "since last",
    "stepped back",
    "new plan",
    "forget the original",
    "moved",
    "old slot",
    "is gone",
];
