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
