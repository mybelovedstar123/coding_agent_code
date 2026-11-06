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
