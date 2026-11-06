//! Contact-channel evidence: for a request that asks for a person's messaging handle, the records that name
//! that person (under any recorded name form) together with the channel, so later corrections and removal
//! instructions reach the deciding model with the value they change. Retrieval only: it selects records, it
//! never reads or writes a reply.
//!
//! Port of `harness/contact_evidence.py`. The Python `render` / `fmt_ts` callback parameters are dropped: the
//! agent always passes `store.render_pair` / `store.fmt_ts`, which are called directly here.

use crate::py;
use crate::pyre::{escape, Re};
use crate::store::{alias_declaration, cmp_content, fmt_ts, render_pair, Pair, UserMemory};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

static HANDLE_REQUEST_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\bhandle\b|\bSignal\b"));
// the channel named loosely, so a typo in a record ("hemdle", "hnale") does not hide it
static CHANNEL_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\bsignal\b|\bh\w{2,4}le\b"));

/// The request asks for a messaging handle.
pub fn asks_for_handle(question: &str) -> bool {
    !question.is_empty() && HANDLE_REQUEST_RE.is_match(question)
}

/// Compiled name patterns, by pattern text (names recur across requests; compiling dominates otherwise).
static NAME_RES: LazyLock<Mutex<HashMap<String, Arc<Re>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn cached_re(pattern: String) -> Arc<Re> {
    let mut cache = NAME_RES.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(pattern)
        .or_insert_with_key(|p| Arc::new(Re::new(p)))
        .clone()
}

static WORD_CHAR_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\A\w\z"));

/// A word character as `\b` sees it (the regex's own `\w`).
fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| WORD_CHAR_RE.is_match(c.encode_utf8(&mut [0u8; 4])))
}

/// `re.search(rf"\b{re.escape(form)}\b", text)`: some occurrence of the literal form (overlapping ones included)
/// with a word boundary at both ends. Checked per occurrence instead of by running the pattern over the whole
/// text: the Unicode `\b` keeps the regex off its fast engines, and the scans over long records dominated.
fn names_form(form: &str, text: &str) -> bool {
    let (Some(first), Some(last)) = (form.chars().next(), form.chars().next_back()) else {
        return cached_re(format!(r"\b{}\b", escape(form))).is_match(text);
    };
    let (first_word, last_word) = (is_word(Some(first)), is_word(Some(last)));
    let mut from = 0;
    while let Some(i) = text[from..].find(form) {
        let start = from + i;
        let end = start + form.len();
        if is_word(text[..start].chars().next_back()) != first_word && is_word(text[end..].chars().next()) != last_word
        {
            return true;
        }
        from = start + first.len_utf8();
    }
    false
}

/// Every recorded name form (full names and aliases) of the persons the request names by either form.
///
/// A Python set: callers only test it for emptiness and membership (`names_any`), so its order never reaches
/// output; the sorted set here is an implementation choice, not a reproduced order.
pub fn person_name_forms(mem: &UserMemory, question: &str) -> BTreeSet<String> {
    let low = question.to_lowercase();
    let mut forms = BTreeSet::new();
    for (alias, people) in &mem.alias_people {
        if names_form(alias, question) {
            forms.insert(alias.clone());
            forms.extend(people.iter().cloned());
        }
    }
    for (full, aliases) in &mem.aliases {
        if low.contains(full.as_str()) {
            forms.extend(aliases.iter().cloned());
            for a in aliases {
                if let Some(people) = mem.alias_people.get(a) {
                    forms.extend(people.iter().filter(|p| p.to_lowercase() == *full).cloned());
                }
            }
        }
    }
    forms
}

/// The text names one of the given name forms (whole words).
pub fn names_any(text: &str, forms: &BTreeSet<String>) -> bool {
    forms.iter().any(|f| names_form(f, text))
}

/// `sorted(out, key=lambda p: (p.ts, p.prompt, p.response))`.
fn oldest_first(mut out: Vec<Pair>) -> Vec<Pair> {
    out.sort_by(cmp_content);
    out
}

/// For a handle request: the records naming the person asked about together with the channel, oldest
/// first; empty for any other request.
pub fn channel_records(mem: &UserMemory, question: &str) -> Vec<Pair> {
    if !asks_for_handle(question) {
        return Vec::new();
    }
    let forms = person_name_forms(mem, question);
    if forms.is_empty() {
        return Vec::new();
    }
    let out = mem
        .pairs
        .values()
        // Both tests are pure: the cheap name test (a literal prefilter) runs first.
        .filter(|p| names_any(&p.prompt, &forms) && CHANNEL_RE.is_match(&p.prompt))
        .cloned()
        .collect();
    oldest_first(out)
}

static REMOVAL_RE: LazyLock<Re> = LazyLock::new(|| {
    Re::new(r"(?i)\b(?:remove|drop|delete|retired?|closed|forget)\b|\bdon['’]t (?:want it on file|hand it out)\b")
});

/// One of the channel records asks for a saved handle to be removed.
pub fn has_removal_instruction(records: &[Pair]) -> bool {
    records.iter().any(|p| REMOVAL_RE.is_match(&p.prompt))
}

pub const REMOVAL_NOTE: &str = concat!(
    "Some of these records are the user's own instructions to remove or stop sharing a saved handle. ",
    "A handle the user asked you to remove is no longer theirs to hand out: do not give it as the current ",
    "handle, and do not repeat it at all, not even as the old or previous handle."
);

/// `json.loads(context.split("\n", 1)[1])` for a context that has a newline; None when there is no context, no
/// newline or the payload does not parse (the Python `except ValueError` path).
fn load_payload(context: Option<&str>) -> Option<Value> {
    let context = context.filter(|c| !c.is_empty())?;
    let (_, body) = context.split_once('\n')?;
    serde_json::from_str(body).ok()
}

/// `f"{payload.get('note', '')} {extra}".strip()`.
fn appended_note(payload: &Map<String, Value>, extra: &str) -> String {
    let note = payload.get("note").map(py::str_value).unwrap_or_default();
    py::strip(&format!("{note} {extra}")).to_string()
}

/// `prefix + "\n" + json.dumps(payload, ensure_ascii=False)`.
fn dump_context(prefix: &str, payload: &Value) -> String {
    format!("{prefix}\n{}", py::dumps(payload, false))
}

/// The memory context with REMOVAL_NOTE appended to its note (the records are unchanged).
///
/// A payload that parses but is not a JSON object raises AttributeError in the Python (never reached: the agent
/// builds every context as an object); here it leaves the context unchanged.
pub fn with_removal_note(context: Option<String>, prefix: &str) -> Option<String> {
    let Some(mut payload) = load_payload(context.as_deref()) else {
        return context;
    };
    let Some(obj) = payload.as_object_mut() else {
        return context;
    };
    let note = appended_note(obj, REMOVAL_NOTE);
    obj.insert("note".into(), Value::String(note));
    Some(dump_context(prefix, &payload))
}
