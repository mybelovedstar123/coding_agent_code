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

// --------------------------------------------------------------------------------------------------------- module

const STOP_WORDS: &str =
    "a an the and or but of to in on at for with from by is are was were be been it its this that these those
my our your their his her we i you they he she me us them now then there here what which who whom whose when where how
just still also not no yes so as if than too very can will would should could do does did done have has had please
before after over about into out up down off again thing one ones back for re regarding quick help remember think
where are we need something work email current live address person whoever owns own owner running holding hold
responsible accountable point things stand moment these days now give tell me who's whose anyone else someone
everyone something anything nobody";

pub static STOP: LazyLock<HashSet<&'static str>> = LazyLock::new(|| py::split_ws(STOP_WORDS).into_iter().collect());

pub static OWNERSHIP: LazyLock<Re> = LazyLock::new(|| {
    Re::new(
        r"(?i)\b(owns?|owner|owned|holding|holds|held|in charge|on point|took over|taken over|takes over|picked up|passed|handed|the handoff on|moved to|hands? (?:it )?(?:over|off)|now runs|running it|holding the thread|'s now|off it|accountable|responsible|person to ask|took it on|stepped back|in the lead|leads it)\b",
    )
});
pub static PERSON: LazyLock<Re> =
    LazyLock::new(|| Re::new(r"\b([A-Z][a-z]+(?:[-'][A-Z]?[a-z]+)? [A-Z][a-z]+(?:[-'][A-Z]?[a-z]+)?)\b"));
