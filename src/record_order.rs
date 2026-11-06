//! Copied notes: a case file whose records hold the same statements several times (one note rendered as a calendar
//! entry, a forwarded text, a list snapshot...). The copies' timestamps say when each copy was filed, not when its
//! event happened, so the order of events has to come from what the records say. Detection is structural
//! (near-equal statements after the identifier); the note is advisory and never names a value.
//!
//! Port of `harness/record_order.py`. Records are (pairID, timestamp text, timestamp, prompt).

use crate::pyre::Re;
use chrono::{DateTime, Utc};
use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

pub const COPY_JACCARD: f64 = 0.7;
pub const MIN_WORDS: usize = 5;
pub static WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z0-9]{3,}"));

/// A record as (pairID, timestamp text, timestamp, prompt).
pub type Record = (String, String, DateTime<Utc>, String);
/// A copy as (pairID, timestamp text, timestamp).
pub type Copy = (String, String, DateTime<Utc>);

/// The statement a record files under the identifier: the text after `<ident>:` up to the end of its line.
pub fn statement(text: &str, ident: &str) -> String {
    let low = text.to_lowercase();
    let needle = format!("{ident}:");
    match low.find(&needle) {
        None => String::new(),
        Some(at) => {
            let rest = &low[at + needle.len()..];
            rest.split('\n').next().unwrap_or("").to_string()
        }
    }
}

pub fn words(text: &str) -> BTreeSet<String> {
    WORD_RE.findall(text).into_iter().collect()
}

/// Same statement up to the copy's typos: word-set overlap at least COPY_JACCARD.
pub fn near_copies(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    if a.len() < MIN_WORDS || b.len() < MIN_WORDS {
        return false;
    }
    let inter = a.intersection(b).count();
    let union = a.union(b).count();
    inter as f64 / union as f64 >= COPY_JACCARD
}

pub const COPIES_NOTE_HEAD: &str = "Some of these records are copies of the same note filed more than once, and the copies disagree on order, so a record's timestamp is when that copy was filed. Copies of one note:";

/// Each statement filed in more than one copy, as [(pairID, timestamp text, timestamp)].
pub fn copy_sets(records: &[Record], ident: &str) -> Vec<Vec<Copy>> {
    let mut groups: Vec<(BTreeSet<String>, Vec<Copy>)> = Vec::new();
    for (pid, stamp, ts, prompt) in records {
        let w = words(&statement(prompt, ident));
        if w.len() < MIN_WORDS {
            continue;
        }
        let copy = (pid.clone(), stamp.clone(), *ts);
        match groups.iter_mut().find(|(rep, _)| near_copies(rep, &w)) {
            Some((_, members)) => members.push(copy),
            None => groups.push((w, vec![copy])),
        }
    }
    groups
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|(_, members)| members)
        .collect()
}
