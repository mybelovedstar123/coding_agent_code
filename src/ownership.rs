//! Who holds a project now, from the user's own notes.
//!
//! A user's story notes about a project mention it many times over months: who started it, who it was passed to, who
//! picked it up, which vendor took over. When a request asks who holds a project now, this module finds the project's
//! name in the request (a run of lower-case request words that recurs, typo-tolerant, in the user's records) and lists,
//! in time order, every sentence of the records that names that project and says who holds, owns, took over or was
//! handed it. The deciding model reads that list next to the retrieved records.
//!
//! Port of `harness/ownership.py`. `difflib.SequenceMatcher(None, a, b).ratio()` is ported below
//! (`sequence_ratio`), autojunk heuristic included, so every fuzzy threshold decides exactly as in Python.
//!
//! Regex notes: Python's `\s` also matches U+001C..U+001F (`str.isspace`), so the patterns here spell it
//! `[\s\x1c-\x1f]`. `\w`/`\b` keep the regex crate's Unicode definition, which differs from Python's only on combining
//! marks, connector punctuation other than `_`, and non-decimal numerics (none occur in the records).

use crate::difflib::sequence_ratio;
use crate::py;
use crate::pyre::Re;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{LazyLock, OnceLock};
