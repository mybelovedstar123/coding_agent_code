//! Case files that define their own field names.
//!
//! Some case files first set out local field conventions ("Local field conventions here: jorailset designates a
//! workstream; lumauset names the accountable owner …", "note that beluxwire is the party that pays us for it; …") and
//! then state facts with those invented names ("For voraenpath the beluxwire on record is Copperfield Systems"). This
//! module reads the definitions from the request's identifier records and returns the glossary plus the records' other
//! statements with each defined name followed by its meaning, so the deciding model reads the facts in plain words.
//!
//! Port of `harness/glossary.py`.

use crate::py;
use crate::pyre::{escape, Re};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::LazyLock;

pub const DEF_VERBS: &str = concat!(
    r"is|are|designates|denotes|names|records|identifies|carries|marks|means|stands for|refers to|",
    r"is the|holds|gives|captures|tracks|labels"
);
pub static DEF_RE: LazyLock<Re> =
    LazyLock::new(|| Re::new(&format!(r"^\s*([a-z][a-z0-9]{{4,}})\s+({DEF_VERBS})\b\s*(.+?)\s*$")));
pub static CLAUSE_SPLIT: LazyLock<Re> =
    LazyLock::new(|| Re::new(&format!(r"[;]|\.\s+|,\s+(?=[a-z][a-z0-9]{{4,}}\s+(?:{DEF_VERBS})\b)")));
pub static INTRO_RE: LazyLock<Re> = LazyLock::new(|| {
    Re::new(concat!(
        r"(?i)(?:conventions here|glossary|note that|field meanings|schema)[^:]*:\s*|before the data lands,?\s*",
        r"note that\s*"
    ))
});
static SCOPE_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\s*(?:within|for|in|under)\b"));
static NON_WORD_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\W+"));
pub const MAX_RECORD_CHARS: usize = 15_000;
pub const GLOSSARY_NOTE: &str = concat!(
    "glossary: the field names these records define for themselves, with their meanings as the records state them; ",
    "glossed_statements are the records' other statements with each defined name followed by its meaning in ",
    "brackets, a host-side annotation of the records: where it differs from the records, the records decide. Answer ",
    "with the value a statement gives for the field asked about, never with the field's own name."
);

/// `prompt[i + len(ident):]` where `i = prompt.lower().find(ident.lower())` (code-point offsets), or the whole
/// prompt when the identifier is absent.
pub(crate) fn after_ident<'a>(prompt: &'a str, ident: &str) -> &'a str {
    let low = prompt.to_lowercase();
    match low.find(&ident.to_lowercase()) {
        None => prompt,
        Some(b) => {
            let start = py::len(&low[..b]) + py::len(ident);
            let skip: usize = prompt.chars().take(start).map(char::len_utf8).sum();
            &prompt[skip..]
        }
    }
}

pub fn statement(prompt: &str, ident: &str) -> String {
    let s = after_ident(prompt, ident);
    let s = py::strip(s.trim_start_matches(':'));
    let s = py::strip(s.trim_end_matches('}'));
    py::strip(s.trim_matches('"')).to_string()
}

/// term -> meaning from a definitions note; empty when the note defines fewer than two names.
pub fn definitions(text: &str) -> IndexMap<String, String> {
    let m = INTRO_RE.search(text);
    let start = match m {
        Some((_, end)) => end,
        None if SCOPE_RE.match_start(text).is_some() => text.find(',').map_or(0, |b| b + 1),
        None => 0,
    };
    let mut out: IndexMap<String, String> = IndexMap::new();
    for clause in CLAUSE_SPLIT.split(&text[start..]) {
        let last = clause.rsplit(':').next().unwrap_or(clause);
        let candidate = py::strip(last).trim_end_matches('.');
        if let Some(d) = DEF_RE.match_start(candidate) {
            let (term, verb, meaning) = (&d[1], &d[2], &d[3]);
            out.entry(term.to_string())
                .or_insert_with(|| py::strip(&format!("{verb} {meaning}")).to_string());
        }
    }
    let need = if m.is_some() { 2 } else { 3 };
    if out.len() >= need {
        out
    } else {
        IndexMap::new()
    }
}

pub fn glossary(records: &[(String, String)], ident: &str) -> IndexMap<String, String> {
    let mut terms: IndexMap<String, String> = IndexMap::new();
    for (_, prompt) in records {
        for (term, meaning) in definitions(&statement(prompt, ident)) {
            terms.entry(term).or_insert(meaning);
        }
    }
    terms
}

/// The defined names as one alternation, longest first (stable: equal lengths keep the glossary's order).
fn terms_re(terms: &IndexMap<String, String>) -> Re {
    let mut names: Vec<&String> = terms.keys().collect();
    names.sort_by_key(|t| std::cmp::Reverse(py::len(t)));
    Re::new(&format!(
        r"\b({})\b",
        names.iter().map(|t| escape(t)).collect::<Vec<_>>().join("|")
    ))
}

/// Each defined name in `text` followed by its meaning in brackets.
///
/// With no terms the Python pattern is `\b()\b` and the substitution raises KeyError (never reached: callers pass
/// a non-empty glossary); here the text is returned unchanged.
pub fn glossed(text: &str, terms: &IndexMap<String, String>) -> String {
    if terms.is_empty() {
        return text.to_string();
    }
    terms_re(terms).sub_with(text, |c| {
        let name = &c[1];
        format!("{name} [= {}]", terms.get(name).map(String::as_str).unwrap_or(""))
    })
}
