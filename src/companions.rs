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

/// `pairs` in a total order (the caller sorts by content key) so the joint lists are deterministic.
pub fn build_index(pairs: &[&Pair]) -> Index {
    let mut phrase_records: HashMap<(String, String), usize> = HashMap::new();
    let mut joint: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in pairs {
        for ph in phrases(&p.prompt) {
            *phrase_records.entry(ph).or_insert(0) += 1;
        }
        let names: BTreeSet<String> = JOINT_RE.findall(&p.prompt).into_iter().collect();
        for name in names {
            joint.entry(name).or_default().push(p.pair_id.clone());
        }
    }
    Index { phrase_records, joint }
}

/// Python's `\w` for one character (`str.isalnum()` or `_`); `char::is_alphanumeric` differs from it only on the
/// Other_Alphabetic marks.
fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// `re.search(rf"\b{name}\b", text)` for a joint-mention name (`[A-Z][a-z]{1,15}`, so its occurrences never overlap
/// and both its ends are word characters), without compiling a pattern per name and anchor.
fn names_word(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

/// Records reached from the anchors through a person they name; none when no anchor shares a rare phrase.
pub fn companion_records<'a>(
    index: &Index,
    pairs: &'a IndexMap<String, Pair>,
    request: &str,
    anchors: &[&Pair],
    exclude: &HashSet<String>,
) -> Vec<&'a Pair> {
    if index.joint.is_empty() {
        return Vec::new();
    }
    let req_phrases = phrases(request);
    let req_words: HashSet<String> = tokenize(request).into_iter().collect();
    // Each anchor's rarest phrase shared with the request; only the anchors at the rarest of these take part.
    let mut rarity: HashMap<&str, usize> = HashMap::new();
    for anchor in anchors {
        let anchor_phrases = phrases(&anchor.prompt);
        let shared = req_phrases.intersection(&anchor_phrases);
        if let Some(r) = shared
            .map(|ph| index.phrase_records.get(ph).copied().unwrap_or(0))
            .min()
        {
            rarity.insert(anchor.pair_id.as_str(), r);
        }
    }
    let best = rarity.values().copied().min().unwrap_or(RARE_PHRASE_RECORDS + 1);
    if best > RARE_PHRASE_RECORDS {
        return Vec::new();
    }
    let mut out: Vec<&'a Pair> = Vec::new();
    let mut seen: HashSet<String> = exclude.clone();
    for anchor in anchors {
        if rarity.get(anchor.pair_id.as_str()) != Some(&best) {
            continue;
        }
        for (name, pids) in &index.joint {
            if !names_word(&anchor.prompt, name) {
                continue;
            }
            let mut places = proper_words(&anchor.prompt);
            places.remove(name);
            for pid in pids {
                let Some(pair) = pairs.get(pid) else { continue };
                if seen.contains(pid) || !tokenize(&pair.prompt).iter().any(|t| req_words.contains(t)) {
                    continue;
                }
                if places.is_disjoint(&proper_words(&pair.prompt)) {
                    continue;
                }
                seen.insert(pid.clone());
                out.push(pair);
            }
        }
    }
    out.truncate(MAX_RECORDS);
    out
}
