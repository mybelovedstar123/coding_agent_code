//! Tool hints: a request whose wording matches what one offered tool is specifically for (when automations run and
//! how often they have run; an edit to an earlier image; arithmetic on numbers the request gives) gets a one-line note
//! naming that tool and its own description. The trigger comes from the request and the offered catalog only; the
//! note is advisory, names no value, and the model still chooses every call.
//!
//! Patterns are `event_state::PyRe`, so `\s`, `\w` and `\b` keep Python's classes.

use crate::difflib::sequence_ratio;
use crate::event_state::PyRe;
use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::Value;
use std::sync::LazyLock;

pub const TOOL_HINT_NOTE: &str = "For this request, the offered tool {tool} is the one made for it ({desc}). Call it \
once with what the request gives and answer from its result.";
pub static NUMBER_RE: LazyLock<PyRe> = LazyLock::new(|| re(r"(?<![\w.])-?\d+(?:[.,]\d+)?(?![\w])"));

fn re(p: &str) -> PyRe {
    PyRe::new(p)
}

static WORDS_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z]+"));

pub fn _words(text: &str) -> Vec<String> {
    WORDS_RE.findall(&text.to_lowercase())
}

// ------------------------------------------------------------------------------------------------------- triggers

pub fn _fuzzy(w: &str, target: &str, ratio: f64) -> bool {
    w == target
        || (py::len(w) >= 4
            && w.chars().next() == target.chars().next()
            && py::len(w).abs_diff(py::len(target)) <= 2
            && sequence_ratio(w, target) >= ratio)
}

pub fn _any(words: &[String], targets: &[&str], ratio: f64) -> bool {
    words.iter().any(|w| targets.iter().any(|t| _fuzzy(w, t, ratio)))
}

fn has_word(words: &[String], word: &str) -> bool {
    words.iter().any(|w| w == word)
}

static SCHED_COUNTING: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(how m\w{1,3} r\w{1,4}|logged|run count|pull up|lay out|show me|what|which)\b"));
static SCHED_SET_UP: LazyLock<PyRe> =
    LazyLock::new(|| re(r"\b(put|set up|make|have|turn)\b[^.?!]*\b(every|each|daily|weekly|nightly)\b"));
static SCHED_EVERY: LazyLock<PyRe> = LazyLock::new(|| re(r"\b(every|each|daily|weekly|nightly)\b"));
static SCHED_ABOUT_RUNS: LazyLock<PyRe> = LazyLock::new(|| {
    re(concat!(
        r"\b((?:due|q\w{3,6}d|lined up|set|scheduled) to run|timed runs?|on a c\w{3,5}e|",
        r"on th\w{2,3} own|on (?:their|its) own|on a t\w{3}r|how m\w{1,3} r\w{0,3}s|run count|",
        r"logged|on repeat|automatic runs?|automatically|automaically|go(?:es)? on their own)\b",
    ))
});
