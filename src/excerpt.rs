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
