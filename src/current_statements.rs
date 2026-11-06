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
const STOP_WORDS: &str =
    "a an the and or but of to in on at for with from by is are was were be been it its this that \
these those
my our your their his her we i you they he she me us them now then there here what which who whom whose when where how
just still also not no yes so as if than too very can will would should could do does did done have has had please
before after over about into out up down off on is it's i'd i'm we're they're one two";
pub static STOP: LazyLock<HashSet<&'static str>> = LazyLock::new(|| py::split_ws(STOP_WORDS).into_iter().collect());
pub const MAX_RECORD_CHARS: usize = 15_000;
pub const CURRENT_NOTE: &str = concat!(
    "current_statements: statements among the records for the identifier in the request that describe a change ",
    "(an update, a revision, a replacement or a handover), each with the earlier statements on the same topic and ",
    "the records they come from. Copies of a note can be refiled, so their filing dates may not show the order in ",
    "which things were said. current_list, where given, is a host-side reading of a list after the change and can ",
    "be wrong; the records themselves decide. When you name the current decision or plan, use the statement's own ",
    "words and verb form as written (not rephrased into another tense or an -ing form)."
);
