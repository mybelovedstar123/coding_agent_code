//! Records that continue a plan through the person it was made with.
//!
//! A note names something the user planned with a person ("Our grape track was the honeymoon trip … that I planned
//! with Sparky"); the later notes about it name only that person ("When Sparky and I first mapped that trip out, we
//! had 8 days in …", "Quick update on the trip Sparky and I planned: …"), never the plan's own name. A request that
//! names the plan retrieves the first note and misses the others, so the model sees the plan without its details.
//!
//! `companion_records` adds them. An anchor is a context record that shares a rare phrase with the request (two
//! adjacent content words found in at most RARE_PHRASE_RECORDS of the user's records); when several do, only those
//! whose shared phrase is the rarest. For each person the anchor names who appears in a first-person joint mention
//! ("<Name> and I"), it adds the records carrying that joint mention which also share a content word with the
//! request and another capitalised word (a place, say) with the anchor; the first MAX_RECORDS found. The index is
//! built once per user from the records and dropped whenever a record changes (`UserMemory.derived`).
//!
//! Port of `harness/companions.py`.

use crate::pyre::Re;
use crate::store::Pair;
use crate::text::tokenize;
use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

pub static JOINT_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\b([A-Z][a-z]{1,15}) and I\b"));
pub const RARE_PHRASE_RECORDS: usize = 2;
pub const MAX_RECORDS: usize = 4;
pub static PROPER_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\b[A-Z][a-z]+\b"));

/// Capitalised words other than the text's first word.
pub fn proper_words(text: &str) -> BTreeSet<String> {
    PROPER_RE
        .find_iter(text)
        .into_iter()
        .filter(|(start, _, _)| *start > 0)
        .map(|(_, _, t)| t.to_string())
        .collect()
}

/// Adjacent pairs of content words (`text.tokenize`).
pub fn phrases(text: &str) -> HashSet<(String, String)> {
    let toks = tokenize(text);
    toks.iter()
        .zip(toks.iter().skip(1))
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect()
}

pub struct Index {
    /// Counter: phrase -> number of records holding it.
    pub phrase_records: HashMap<(String, String), usize>,
    /// person -> pair ids whose record holds "<person> and I", in record order (sorted by person, as the Python
    /// iterates it).
    pub joint: BTreeMap<String, Vec<String>>,
}
