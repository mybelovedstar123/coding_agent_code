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
/// The Python sentence splitter, `(?<=[.;!?])\s+|\s+…\s+|\n+`. `sentences` splits with an equivalent pattern that
/// needs no lookbehind, so the regex crate's automata run it instead of the backtracking engine.
pub static SENT: LazyLock<Re> =
    LazyLock::new(|| Re::new(r"(?<=[.;!?])[\s\x1c-\x1f]+|[\s\x1c-\x1f]+…[\s\x1c-\x1f]+|\n+"));
/// `SENT` with the lookbehind's character consumed: a match starting on one of `.;!?` is a split starting one
/// character later. The leftmost match is the same: a first-branch match at p-1 is one of SENT's at p, and no other
/// branch of either pattern can start on the punctuation character.
static SENT_FAST: LazyLock<Re> = LazyLock::new(|| Re::new(r"[.;!?][\s\x1c-\x1f]+|[\s\x1c-\x1f]+…[\s\x1c-\x1f]+|\n+"));

/// `SENT.split(text)`.
pub fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut last = 0;
    for (start, end, m) in SENT_FAST.find_iter(text) {
        let start = if m.starts_with(['.', ';', '!', '?']) {
            start + 1
        } else {
            start
        };
        out.push(&text[last..start]);
        last = end;
    }
    out.push(&text[last..]);
    out
}

static TOKEN_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[a-z0-9][a-z0-9'-]*"));

pub fn tokens(text: &str) -> Vec<String> {
    TOKEN_RE
        .find_iter(&text.to_lowercase())
        .into_iter()
        .map(|(_, _, t)| t.to_string())
        .collect()
}

/// Whether `phrase` occurs in the token list, typo-tolerant: an n-token window equal to it, or starting with the
/// same character and at least `ratio` similar (default 0.82 for 3+ words, 0.88 otherwise; `None` or 0.0 is the
/// default, as Python's `ratio or ...`).
pub fn fuzzy_in(phrase: &str, text_tokens: &[String], ratio: Option<f64>) -> bool {
    let n = py::split_ws(phrase).len();
    let ratio = match ratio {
        Some(r) if r != 0.0 => r,
        _ => {
            if n >= 3 {
                0.82
            } else {
                0.88
            }
        }
    };
    if text_tokens.len() < n {
        return false;
    }
    let first = py::head(phrase, 1);
    for i in 0..text_tokens.len() + 1 - n {
        let win = text_tokens[i..i + n].join(" ");
        if win == phrase || (py::head(&win, 1) == first && sequence_ratio(&win, phrase) >= ratio) {
            return true;
        }
    }
    false
}

pub const MAX_PHRASE_RECORDS: usize = 25;
pub const BOUNDARY_WORDS: [&str; 5] = ["thread", "saga", "situation", "business", "project"];

/// A word that names the conversation rather than the project (thread, saga, …), typos included.
pub fn thread_like(word: &str) -> bool {
    let wlen = py::len(word);
    BOUNDARY_WORDS.iter().any(|b| {
        word == *b
            || (wlen >= 4
                && word.chars().next() == b.chars().next()
                && wlen.abs_diff(py::len(b)) <= 2
                && sequence_ratio(word, b) >= 0.7)
    })
}

/// Handover sentences as (record timestamp, sentence), in time order.
pub type Chain = Vec<(String, String)>;
/// A record's handover sentence: (stripped sentence, its tokens).
type Handover = (String, Vec<String>);

/// Records tokenised once: (timestamp, text, token list, token set).
pub struct Index {
    pub recs: Vec<(String, String, Vec<String>)>,
    pub sets: Vec<HashSet<String>>,
    /// Per record, built on first use: its handover sentences as (stripped sentence, tokens), the sentences that pass
    /// the phrase-independent tests of `chain` (no quotation mark, OWNERSHIP and PERSON match), in record order. A
    /// cache only: `chain` and `sources` decide exactly as the Python, which re-splits the record on every call.
    handovers: Vec<OnceLock<Vec<Handover>>>,
}

impl Index {
    /// Records as (timestamp, text), sorted (stably) by timestamp.
    pub fn new(records: &[(String, String)]) -> Index {
        let mut sorted: Vec<&(String, String)> = records.iter().collect();
        sorted.sort_by(|x, y| x.0.cmp(&y.0));
        let recs: Vec<(String, String, Vec<String>)> = sorted
            .into_iter()
            .map(|(ts, t)| (ts.clone(), t.clone(), tokens(t)))
            .collect();
        let sets = recs.iter().map(|(_, _, tk)| tk.iter().cloned().collect()).collect();
        let handovers = recs.iter().map(|_| OnceLock::new()).collect();
        Index { recs, sets, handovers }
    }

    /// Record `i`'s handover sentences (see `handovers`).
    fn handovers(&self, i: usize) -> &[Handover] {
        self.handovers[i].get_or_init(|| {
            sentences(&self.recs[i].1)
                .into_iter()
                .filter(|s| !s.contains('"') && !s.contains('\u{201c}'))
                .filter(|s| OWNERSHIP.is_match(s) && PERSON.is_match(s))
                .map(|s| (py::strip(s).to_string(), tokens(s)))
                .collect()
        })
    }

    /// Records holding all but at most one of the phrase's words exactly (one typo'd word allowed).
    pub fn candidates(&self, phrase: &str) -> Vec<usize> {
        let words = py::split_ws(phrase);
        let need = words.len().saturating_sub(1).max(1);
        self.sets
            .iter()
            .enumerate()
            .filter(|(_, st)| words.iter().filter(|w| st.contains(**w)).count() >= need)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn occurs(&self, phrase: &str) -> usize {
        self.candidates(phrase)
            .into_iter()
            .filter(|&i| fuzzy_in(phrase, &self.recs[i].2, None))
            .count()
    }

    pub fn chain(&self, phrase: &str) -> Chain {
        let mut out = Vec::new();
        for i in self.candidates(phrase) {
            let ts = &self.recs[i].0;
            for (s, toks) in self.handovers(i) {
                if fuzzy_in(phrase, toks, None) {
                    out.push((ts.clone(), s.clone()));
                }
            }
        }
        out
    }

    /// The project phrase of a holder request and its handover sentences: the longest request n-gram (5 down to 2
    /// words) that recurs in 2..=MAX_PHRASE_RECORDS records and has a chain, the one with the most sentences (then
    /// the greatest phrase) at that length.
    pub fn best_chain(&self, request: &str) -> (Option<String>, Chain) {
        if !asks_holder(request) {
            return (None, Vec::new());
        }
        let lowers = lower_grams(request);
        let toks = tokens(request);
        for n in [5usize, 4, 3, 2] {
            let mut found: Vec<(usize, String, Chain)> = Vec::new();
            if toks.len() >= n {
                for i in 0..toks.len() + 1 - n {
                    let gram = &toks[i..i + n];
                    if STOP.contains(gram[0].as_str())
                        || STOP.contains(gram[n - 1].as_str())
                        || gram.iter().filter(|w| !STOP.contains(w.as_str())).count() < 2
                    {
                        continue;
                    }
                    if thread_like(&gram[0]) || thread_like(&gram[n - 1]) {
                        continue;
                    }
                    let phrase = gram.join(" ");
                    if !lowers.contains(&phrase) {
                        continue;
                    }
                    let occ = self.occurs(&phrase);
                    if !(2..=MAX_PHRASE_RECORDS).contains(&occ) {
                        continue;
                    }
                    let ch = self.chain(&phrase);
                    if !ch.is_empty() {
                        found.push((ch.len(), phrase, ch));
                    }
                }
            }
            // max(found): the first of the greatest (len, phrase, chain) tuples.
            let mut best: Option<&(usize, String, Chain)> = None;
            for f in &found {
                if best.is_none_or(|b| (f.0, &f.1, &f.2) > (b.0, &b.1, &b.2)) {
                    best = Some(f);
                }
            }
            if let Some((_, phrase, ch)) = best {
                return (Some(phrase.clone()), ch.clone());
            }
        }
        (None, Vec::new())
    }
}

pub const HOLDER_WORDS: [&str; 6] = ["owns", "owner", "holding", "accountable", "responsible", "running"];
pub const WH_WORDS: [&str; 12] = [
    "who", "whom", "why", "what", "when", "where", "which", "while", "whole", "white", "whip", "whim",
];
pub static HOLDER_PHRASES: LazyLock<Re> = LazyLock::new(|| {
    Re::new(
        r"(?i)\b(on p\w{3,4}t|in charge|has (?:it|the \w+) now|who has|ended up with|who do i chase|chase about|who (?:o\w{1,3}|h\w{2,4}s) it|whose is it|owner'?s?)\b",
    )
});
static WHOSE_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\bwhose\b"));
static RAW_WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[A-Za-z][A-Za-z'-]*"));

/// The request asks who holds something now (typo-tolerant on the holder words).
pub fn asks_holder(request: &str) -> bool {
    let raw = RAW_WORD_RE.findall(request);
    let internal = ["internal", "internally"];
    if raw.iter().any(|w| {
        let lw = w.to_lowercase();
        lw.starts_with("int")
            && internal
                .iter()
                .map(|t| py::len(w).abs_diff(py::len(t)))
                .min()
                .unwrap_or(0)
                <= 2
            && internal
                .iter()
                .map(|t| sequence_ratio(&lw, t))
                .fold(f64::NEG_INFINITY, f64::max)
                >= 0.85
    }) {
        return false;
    }
    if HOLDER_PHRASES.is_match(request) || WHOSE_RE.is_match(request) {
        return true;
    }
    for w in &raw {
        let lw = w.to_lowercase();
        let n = py::len(&lw);
        if (3..=6).contains(&n)
            && lw.starts_with("wh")
            && !WH_WORDS.contains(&lw.as_str())
            && sequence_ratio(&lw, "whose") >= 0.6
        {
            return true;
        }
        if *w != lw {
            continue;
        }
        // re.sub(r"'s$", "", w): the words carry no newline, so `$` is the end.
        let w = w.strip_suffix("'s").unwrap_or(w);
        let wl = py::len(w);
        for h in HOLDER_WORDS {
            let hl = py::len(h);
            if w == h
                || (wl >= 4
                    && wl.abs_diff(hl) <= 1
                    && w.chars().next() == h.chars().next()
                    && sequence_ratio(w, h) >= if hl >= 8 { 0.7 } else { 0.75 })
            {
                return true;
            }
        }
    }
    false
}

static GRAM_WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[A-Za-z0-9][A-Za-z0-9'-]*"));

/// Phrases written in lower case in the request (project names), as token strings.
pub fn lower_grams(request: &str) -> HashSet<String> {
    let raw = GRAM_WORD_RE.findall(request);
    let ok: Vec<Option<String>> = raw
        .iter()
        .map(|w| {
            if *w == w.to_lowercase() {
                Some(w.to_lowercase())
            } else {
                None
            }
        })
        .collect();
    let mut out = HashSet::new();
    for n in [2usize, 3, 4, 5] {
        if ok.len() < n {
            continue;
        }
        for i in 0..ok.len() + 1 - n {
            let window = &ok[i..i + n];
            if window.iter().all(|w| w.as_deref().is_some_and(|s| !s.is_empty())) {
                out.insert(
                    window
                        .iter()
                        .map(|w| w.as_deref().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
    }
    out
}

pub const NAME: &str = r"([A-Z][a-z]+(?:[-'][A-Z]?[a-z]+)? [A-Z][a-z]+(?:[-'][A-Z]?[a-z]+)?)";

pub static TARGETS: LazyLock<Vec<Re>> = LazyLock::new(|| {
    let ws = r"[\s\x1c-\x1f]";
    [
        format!(r"\b(?:handed|passed\b.*?|moved|over|went){ws}+to{ws}+{NAME}"),
        format!(r":{ws}*{NAME}{ws}+to{ws}+{NAME}"),
        format!(r"\bis{ws}+{NAME}'s{ws}+now"),
        format!(
            r"{NAME}{ws}+(?:now{ws}+owns|is{ws}+the{ws}+owner{ws}+now|picked{ws}+up|took{ws}+over|took{ws}+it{ws}+on|has{ws}+taken{ws}+over|now{ws}+runs|is{ws}+the{ws}+one{ws}+accountable)"
        ),
        format!(
            r"\bwith{ws}+{NAME}{ws}+(?:in{ws}+charge|as{ws}+the{ws}+person{ws}+holding|holding|on{ws}+point)"
        ),
        format!(r"\bball{ws}+is{ws}+with{ws}+{NAME}"),
    ]
    .iter()
    .map(|p| Re::new(p))
    .collect()
});

const ORG_WORDS_TEXT: &str =
    "consulting logistics lodge inn box media catering clinic clinics health rooms company partners capital
supply labs studio studios systems group holdings ventures industries brands collective guild workshop atelier press
analytics digital energy foods interactive textiles cooperative works services hotel hotels removals movers club
academy school centre center";

pub static ORG_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| py::split_ws(ORG_WORDS_TEXT).into_iter().collect());

pub fn is_person(name: &str) -> bool {
    let last = py::split_ws(name).last().map(|w| w.to_lowercase()).unwrap_or_default();
    !ORG_WORDS.contains(last.as_str())
}

/// The last group of each match of `pat` in `s`, in match order.
fn last_groups(pat: &Re, s: &str) -> Vec<String> {
    let last = pat.inner().captures_len() - 1;
    pat.captures_iter(s)
        .iter()
        .filter_map(|c| c.get(last).map(|m| m.as_str().to_string()))
        .collect()
}
