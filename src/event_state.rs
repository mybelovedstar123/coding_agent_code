//! Event histories in stored records -> the state they describe at an effective step.
//!
//! Records of an event history state their own rules (events apply by effective step, not document order; an
//! assignment replaces a field's value; add/remove change only the named member of a set; state persists until
//! changed). The events come as tables (JSON lists of objects, CSV, markdown tables with entity / field / value /
//! effective step / operation columns) or as sentences that quote the entity, field and value and name the step. This
//! module reads the events, applies those rules up to a step, and returns every entity's fields at that step, so the
//! deciding model sees the state the records describe next to the records themselves.
//!
//! Python semantics carried here (private helpers, see the report): `json.loads` (NaN/Infinity, arbitrary-precision
//! ints, the 4300-digit int limit), `int()` of a str/float, `str.splitlines()`, the `csv` module's reader state machine
//! with `DictReader`, and Python's `\s`/`\w`/`\b` classes in patterns (`py_pattern`).

use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

pub const Q: &str = r#""([^"]+)""#;
pub static STEP_RE: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?i)\bstep\s+(\d+)\b"));
pub static SENT_SPLIT: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?<=[.;])\s+(?=[A-Z\[])|\n+"));
pub const MIN_EVENTS: usize = 10;
pub const EVENT_STATE_NOTE: &str = "event_state: a host-side reading of the event history in the records for the \
identifier in the request, replayed up to effective step {step} under the rules those records state (events by \
effective step, not document order; an assignment replaces the field's value; add/remove change only the named member \
of a set; state persists until changed), with every entity's fields at that step and sets as lists. It is an \
unverified candidate, not an answer, and can be wrong: check what you rely on against the records themselves, and \
where they differ, the records decide.";

static TAG_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"<[^>]+>"));
static NON_ALNUM_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[^a-z0-9]+"));
static JSON_LIST_RE: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?s)\[\s*\{.*?\}\s*\]"));
static QUOTED_RE: LazyLock<Re> = LazyLock::new(|| Re::new(Q));
static ADD_REMOVE_RE: LazyLock<PyRe> = LazyLock::new(|| {
    PyRe::new(&format!(
        r"(?i)\b(add|remove)\s+{Q}\s+(?:to|from)\s+the\s+{Q}\s+set\s+of\s+{Q}"
    ))
});
static DIGIT_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"^\d$"));

/// Python's `\w` class body (str patterns): `str.isalnum()` (letters and every numeric category) or `_`.
const PY_WORD: &str = r"\p{L}\p{N}_";

/// A Python `re` pattern rewritten where the engines' classes differ: `\s` also takes U+001C..U+001F, `\w` (and so
/// `\b`) is letters, numbers and `_` only (no combining marks, no joiners, no other connector punctuation).
pub(crate) fn py_pattern(p: &str) -> String {
    let word = format!("[{PY_WORD}]");
    let boundary = format!("(?:(?<!{word})(?={word})|(?<={word})(?!{word}))");
    let mut out = String::with_capacity(p.len() * 2);
    let mut in_class = false;
    let mut chars = p.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(n) = chars.next() else {
                out.push(c);
                break;
            };
            match (n, in_class) {
                ('w', false) => out.push_str(&word),
                ('w', true) => out.push_str(PY_WORD),
                ('s', false) => out.push_str(r"[\s\x1c-\x1f]"),
                ('s', true) => out.push_str(r"\s\x1c-\x1f"),
                ('b', false) => out.push_str(&boundary),
                _ => {
                    out.push('\\');
                    out.push(n);
                }
            }
            continue;
        }
        out.push(c);
        if in_class {
            if c == ']' {
                in_class = false;
            }
        } else if c == '[' {
            in_class = true;
            if chars.peek() == Some(&'^') {
                out.push(chars.next().unwrap_or('^'));
            }
            if chars.peek() == Some(&']') {
                out.push_str("\\]");
                chars.next();
            }
        }
    }
    out
}

/// Whether Rust's and Python's `\w` and `\s` agree on a character: ASCII other than U+001C..U+001F, Latin-1 other
/// than the superscripts and fractions (category No), Latin Extended-A/B, and General Punctuation other than the
/// joiners and the connector punctuation.
fn classes_agree(c: char) -> bool {
    match c as u32 {
        0x1c..=0x1f => false,
        0..=0x7f => true,
        0xb2 | 0xb3 | 0xb9 | 0xbc..=0xbe => false,
        0xa0..=0x24f => true,
        0x200c | 0x200d | 0x203f | 0x2040 | 0x2054 => false,
        0x2000..=0x206f => true,
        _ => false,
    }
}

/// A Python pattern compiled twice: as written (Rust's classes) and through `py_pattern` (Python's classes). Text
/// whose every character the two class sets treat alike runs on the first, which has no lookaround for `` and so
/// stays on the fast engine; any other text runs on the second. Both give Python's result.
pub struct PyRe {
    fast: Re,
    exact: Re,
}

impl PyRe {
    pub fn new(pattern: &str) -> Self {
        PyRe {
            fast: Re::new(pattern),
            exact: Re::new(&py_pattern(pattern)),
        }
    }

    /// The compiled pattern to run on `text`.
    pub fn on(&self, text: &str) -> &Re {
        if text.chars().all(classes_agree) {
            &self.fast
        } else {
            &self.exact
        }
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.on(text).is_match(text)
    }

    pub fn captures<'a>(&self, text: &'a str) -> Option<fancy_regex::Captures<'a>> {
        self.on(text).captures(text)
    }

    pub fn captures_iter<'a>(&self, text: &'a str) -> Vec<fancy_regex::Captures<'a>> {
        self.on(text).captures_iter(text)
    }

    pub fn match_start<'a>(&self, text: &'a str) -> Option<fancy_regex::Captures<'a>> {
        self.on(text).match_start(text)
    }

    pub fn find_iter<'a>(&self, text: &'a str) -> Vec<(usize, usize, &'a str)> {
        self.on(text).find_iter(text)
    }

    pub fn findall(&self, text: &str) -> Vec<String> {
        self.on(text).findall(text)
    }

    pub fn split<'a>(&self, text: &'a str) -> Vec<&'a str> {
        self.on(text).split(text)
    }
}

// ------------------------------------------------------------------------------------------- Python value semantics

/// A Python int (arbitrary precision): sign and decimal digits without leading zeros ("0" for zero).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyInt {
    neg: bool,
    digits: String,
}

/// CPython's default `sys.int_info.default_max_str_digits`.
const INT_MAX_STR_DIGITS: usize = 4300;

impl PyInt {
    fn new(neg: bool, digits: &str) -> PyInt {
        let d = digits.trim_start_matches('0');
        if d.is_empty() {
            PyInt {
                neg: false,
                digits: "0".into(),
            }
        } else {
            PyInt { neg, digits: d.into() }
        }
    }

    pub fn from_i64(v: i64) -> PyInt {
        PyInt::new(v < 0, &v.unsigned_abs().to_string())
    }
}
