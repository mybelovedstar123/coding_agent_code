//! Query-focused excerpts of long memory records.
//!
//! Real conversation records are often long and mostly irrelevant to any one question. Instead of truncating a
//! record by position (which can cut away the one sentence that matters), keep the sentences that match the query
//! terms — tolerating typos — plus their immediate neighbours, in original order.

use crate::py;
use crate::pyre::Re;
use crate::text::tokenize;
use indexmap::IndexMap;
use std::collections::BTreeSet;
use std::sync::LazyLock;

static SENT_SPLIT: LazyLock<Re> = LazyLock::new(|| Re::new("(?<=[.!?;])\\s+(?=[A-Z0-9\"'\u{201c}(])|\\n+"));
static FACT_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\d|@"));

pub fn split_sentences(text: &str) -> Vec<String> {
    SENT_SPLIT
        .split(text)
        .into_iter()
        .map(py::strip)
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect()
}

/// True when a and b differ by at most one edit or one adjacent swap (code points).
fn edit_distance_le1(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (mut a, mut b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (la, lb) = (a.len(), b.len());
    if la.abs_diff(lb) > 1 {
        return false;
    }
    if la == lb {
        let diffs: Vec<usize> = (0..la).filter(|&i| a[i] != b[i]).collect();
        if diffs.len() == 1 {
            return true;
        }
        return diffs.len() == 2
            && diffs[1] == diffs[0] + 1
            && a[diffs[0]] == b[diffs[1]]
            && a[diffs[1]] == b[diffs[0]];
    }
    if la > lb {
        std::mem::swap(&mut a, &mut b);
    }
    let (mut i, mut j, mut skipped) = (0, 0, false);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
            j += 1;
        }
    }
    true
}

/// 1.0 exact, 0.8 one-typo match for tokens of 5+ chars, else 0.
pub fn fuzzy_match(qtok: &str, tok: &str) -> f64 {
    if qtok == tok {
        return 1.0;
    }
    if py::len(qtok) >= 5
        && py::len(tok) >= 4
        && qtok.chars().next() == tok.chars().next()
        && edit_distance_le1(qtok, tok)
    {
        return 0.8;
    }
    0.0
}

/// For each query token, the vocabulary tokens it matches and their weight (vocabulary in insertion order).
pub fn expand_terms<'a>(
    qtoks: &[String],
    vocab: impl Fn() -> Box<dyn Iterator<Item = &'a str> + 'a>,
    contains: impl Fn(&str) -> bool,
) -> IndexMap<String, IndexMap<String, f64>> {
    let mut out = IndexMap::new();
    for q in qtoks {
        let mut matches = IndexMap::new();
        if contains(q) {
            matches.insert(q.clone(), 1.0);
        }
        let ql = py::len(q);
        if ql >= 5 {
            for tok in vocab() {
                if tok != q && py::len(tok).abs_diff(ql) <= 1 && tok.chars().next() == q.chars().next() {
                    let w = fuzzy_match(q, tok);
                    if w != 0.0 {
                        matches.insert(tok.to_string(), w);
                    }
                }
            }
        }
        out.insert(q.clone(), matches);
    }
    out
}

/// Inverse document frequency over a document-frequency table of `n` documents.
#[derive(Clone)]
pub struct Idf<'a> {
    pub df: &'a IndexMap<String, i64>,
    pub n: usize,
}
