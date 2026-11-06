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

static WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z][a-z'-]+"));
static TOKEN_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z0-9:'-]+"));
/// The whole-word pattern of each change and aside cue, compiled once.
static CUE_RES: LazyLock<HashMap<&'static str, Re>> = LazyLock::new(|| {
    CHANGE_CUES
        .iter()
        .chain(&["separately", "unrelated"])
        .map(|c| (*c, cue_re(c)))
        .collect()
});

fn cue_re(cue: &str) -> Re {
    Re::new(&format!(r"(?<![a-z]){}(?![a-z])", escape(cue)))
}

/// `{w for w in re.findall(r"[a-z][a-z'-]+", text.lower()) if w not in STOP and len(w) > 2}`.
///
/// A Python set: only intersected and tested for emptiness, so its order never reaches output.
pub fn words(text: &str) -> HashSet<String> {
    WORD_RE
        .findall(&text.to_lowercase())
        .into_iter()
        .filter(|w| !STOP.contains(w.as_str()) && py::len(w) > 2)
        .collect()
}

/// `cue` in `low` as whole words; a cue of 9+ letters also matches a typo'd form (same first letter, ratio
/// >= 0.85).
pub fn has_phrase(low: &str, cue: &str) -> bool {
    let found = match CUE_RES.get(cue) {
        Some(re) => re.is_match(low),
        None => cue_re(cue).is_match(low),
    };
    if found {
        return true;
    }
    if py::len(cue) < 9 {
        return false;
    }
    let toks = TOKEN_RE.findall(low);
    let n = py::split_ws(cue).len();
    if toks.len() + 1 < n + 1 {
        return false;
    }
    let first = py::head(cue, 1);
    let cue_len = py::len(cue);
    for i in 0..(toks.len() + 1 - n) {
        let win = toks[i..i + n].join(" ");
        if py::head(&win, 1) != first {
            continue;
        }
        // The ratio is at most 2·min(len)/(len + len) (every match is shared by both texts); a window that cannot
        // reach the threshold skips the matcher. Exact: the bound uses the ratio's own formula.
        let win_len = py::len(&win);
        if 2.0 * win_len.min(cue_len) as f64 / (win_len + cue_len) as f64 >= 0.85 && sequence_ratio(&win, cue) >= 0.85 {
            return true;
        }
    }
    false
}

pub fn is_aside(stmt: &str) -> bool {
    let low = stmt.to_lowercase();
    has_phrase(py::head(&low, 14), "separately") || has_phrase(py::head(&low, 12), "unrelated")
}

pub fn is_change(stmt: &str) -> bool {
    if is_aside(stmt) {
        return false;
    }
    let low = stmt.to_lowercase();
    CHANGE_CUES.iter().any(|c| has_phrase(&low, c))
}

/// The note itself: the text after the identifier, without wrapping quotes.
pub fn statement(prompt: &str, ident: &str) -> String {
    let s = crate::glossary::after_ident(prompt, ident);
    let s = py::strip(s.trim_start_matches(':'));
    py::strip(s.trim_matches('"')).to_string()
}

/// One distinct note and the pairIDs that carry it.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub text: String,
    pub records: Vec<String>,
}

/// Distinct notes (copies merged by text similarity >= 0.85) with the pairIDs that carry them.
pub fn notes_of(records: &[(String, String)], ident: &str) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    for (pid, prompt) in records {
        let s = statement(prompt, ident);
        let low = s.to_lowercase();
        match notes
            .iter_mut()
            .find(|n| sequence_ratio(&n.text.to_lowercase(), &low) >= 0.85)
        {
            Some(n) => n.records.push(pid.clone()),
            None => notes.push(Note {
                text: s,
                records: vec![pid.clone()],
            }),
        }
    }
    notes
}
