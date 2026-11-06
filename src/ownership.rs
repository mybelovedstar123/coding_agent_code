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

/// The person the latest handover sentence names as the new holder (organisations skipped).
pub fn first_named_holder(chain: &[(String, String)]) -> Option<String> {
    for (_, s) in chain.iter().rev() {
        for pat in TARGETS.iter() {
            for name in last_groups(pat, s) {
                if is_person(&name) {
                    return Some(name);
                }
            }
        }
    }
    None
}

pub const OWNERSHIP_NOTE: &str = "ownership: every sentence in the user's records that names {project} and says who holds, owns, took over or was handed it, quoted from the records in time order (record date, then order within the record), with those records shown in full. holder_now, where given, is a host-side reading of the latest of these sentences and can be wrong; the records themselves decide.";

/// `Index` that also keeps each record's pairID, so the handover sentences can name their source records.
pub struct IndexV2 {
    pub index: Index,
    pub pids: Vec<String>,
    pub orgs: HashSet<String>,
}

impl Deref for IndexV2 {
    type Target = Index;
    fn deref(&self) -> &Index {
        &self.index
    }
}

impl IndexV2 {
    /// Records as (pairID, timestamp text, text).
    pub fn new(records: &[(String, String, String)]) -> IndexV2 {
        let mut ordered: Vec<&(String, String, String)> = records.iter().collect();
        // time, then text: never insertion order (D15)
        ordered.sort_by(|x, y| (&x.1, &x.2).cmp(&(&y.1, &y.2)));
        let pairs: Vec<(String, String)> = ordered.iter().map(|(_, ts, t)| (ts.clone(), t.clone())).collect();
        IndexV2 {
            index: Index::new(&pairs),
            pids: ordered.iter().map(|(pid, _, _)| pid.clone()).collect(),
            orgs: orgs_in(ordered.iter().map(|(_, _, t)| t.as_str())),
        }
    }

    /// pairIDs of the records the phrase's handover sentences come from, in time order.
    pub fn sources(&self, phrase: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for i in self.candidates(phrase) {
            // The first handover sentence naming the phrase decides (the Python breaks there).
            if self.handovers(i).iter().any(|(_, toks)| fuzzy_in(phrase, toks, None)) && !out.contains(&self.pids[i]) {
                out.push(self.pids[i].clone());
            }
        }
        out
    }
}

/// The memory context with `ownership` (the project's handover sentences, the records they come from shown in full
/// in `memories`, an excerpt of them replaced) and OWNERSHIP_NOTE added when the request asks who holds a project the
/// records name; unchanged otherwise. `full_record(pairID)` returns that record's {"pairID", "timestamp", "text"}.
/// holder_now is the person the latest handover sentence names (`holder_now`).
///
/// A payload that is valid JSON but not an object makes the Python raise (an uncaught TypeError/AttributeError);
/// it never occurs (the agent builds every context), and here the context is returned unchanged.
pub fn with_ownership(
    context: Option<String>,
    prefix: &str,
    index: &IndexV2,
    request: &str,
    full_record: impl Fn(&str) -> Value,
) -> Option<String> {
    let ctx = match context {
        Some(c) if !c.is_empty() && c.contains('\n') => c,
        other => return other,
    };
    let (phrase, chain) = index.best_chain(request);
    if chain.is_empty() {
        return Some(ctx);
    }
    let phrase = phrase.unwrap_or_default();
    let body = ctx.split_once('\n').map(|(_, b)| b).unwrap_or("");
    let mut payload: Map<String, Value> = match serde_json::from_str::<Value>(body) {
        Ok(Value::Object(m)) => m,
        _ => return Some(ctx),
    };
    let old_note = payload.get("note").map(py::str_value).unwrap_or_default();
    let note = format!("{old_note} {}", OWNERSHIP_NOTE.replace("{project}", &phrase));
    payload.insert("note".into(), Value::String(py::strip(&note).to_string()));
    let mut own = Map::new();
    own.insert("project".into(), Value::String(phrase.clone()));
    own.insert(
        "statements".into(),
        Value::Array(
            chain
                .iter()
                .map(|(ts, s)| {
                    let mut m = Map::new();
                    m.insert("timestamp".into(), Value::String(ts.clone()));
                    m.insert("text".into(), Value::String(s.clone()));
                    Value::Object(m)
                })
                .collect(),
        ),
    );
    // `payload["ownership"] = own` happens here, ahead of `memories`; its keys are filled below.
    payload.insert("ownership".into(), Value::Null);
    let holder = holder_now(&chain, &index.orgs);
    // Key order of the payload is kept exactly as the earlier two-pass build produced it (holder_now ahead of
    // source_records when the plain reading `first_named_holder` also names a holder), so model requests are
    // unchanged.
    let early = holder.is_some() && first_named_holder(&chain).is_some();
    if early {
        own.insert("holder_now".into(), Value::String(holder.clone().unwrap_or_default()));
    }
    let pids = index.sources(&phrase);
    let mut items: indexmap::IndexMap<String, Value> = indexmap::IndexMap::new();
    if let Some(Value::Array(mems)) = payload.get("memories") {
        for it in mems {
            if let Value::Object(m) = it {
                let key = m.get("pairID").map(py::str_value).unwrap_or_else(|| "None".into());
                items.insert(key, it.clone());
            }
        }
    }
    for pid in &pids {
        items.insert(pid.clone(), full_record(pid));
    }
    let mut mems: Vec<Value> = items.into_values().collect();
    let ts_key = |it: &Value| match it.get("timestamp") {
        Some(v) => py::str_value(v),
        None => String::new(),
    };
    mems.sort_by_key(ts_key);
    payload.insert("memories".into(), Value::Array(mems));
    own.insert(
        "source_records".into(),
        Value::Array(pids.into_iter().map(Value::String).collect()),
    );
    if let Some(h) = holder.filter(|h| !h.is_empty() && !early) {
        own.insert("holder_now".into(), Value::String(h));
    }
    payload.insert("ownership".into(), Value::Object(own));
    Some(format!("{prefix}\n{}", py::dumps(&Value::Object(payload), false)))
}

pub static ORG_MARKERS: LazyLock<Vec<Re>> = LazyLock::new(|| {
    let ws = r"[\s\x1c-\x1f]";
    vec![
        Re::new(&format!(
            r"(?i:\b(?:works?|worked|working) at|\bused to w\w{{1,3}}k at){ws}+{NAME}"
        )),
        Re::new(&format!(r"{NAME}{ws}+f\w{{1,3}}l+ through")),
        Re::new(&format!(
            r"f\w{{1,3}}l+ through,?{ws}+{NAME}{ws}+(?:took|has taken|picked)"
        )),
    ]
});

/// Names the records mark as organisations ("works at X", "after X fell through", "took over … from X" for a
/// supplier swap).
pub fn orgs_in<'a>(texts: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    let mut out = HashSet::new();
    for t in texts {
        for pat in ORG_MARKERS.iter() {
            for c in pat.captures_iter(t) {
                if let Some(m) = c.get(1) {
                    out.insert(m.as_str().to_string());
                }
            }
        }
    }
    out
}

static SWAP: LazyLock<Re> = LazyLock::new(|| {
    Re::new(&format!(
        r"\b(?:took over|has taken over|taken over)\b[^.;]*?\bfrom[\s\x1c-\x1f]+{NAME}"
    ))
});

/// The person the latest handover sentence names as the new holder, skipping names the records mark as
/// organisations, and skipping a takeover "A took over … from B" whose B never held the project earlier in the chain
/// (a supplier swap, not a handover).
pub fn holder_now(chain: &[(String, String)], orgs: &HashSet<String>) -> Option<String> {
    for k in (0..chain.len()).rev() {
        let s = &chain[k].1;
        let earlier = chain[..k].iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" ");
        if let Some(c) = SWAP.captures(s) {
            let b = c.get(1).map(|m| m.as_str()).unwrap_or("");
            if !earlier.contains(b) {
                continue;
            }
        }
        for pat in TARGETS.iter() {
            for name in last_groups(pat, s) {
                if is_person(&name) && !orgs.contains(&name) {
                    return Some(name);
                }
            }
        }
    }
    None
}
