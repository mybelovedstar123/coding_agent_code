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

// ---- second hop for "the current owner's email": email records of the people the retrieved records name
static EMAIL_REQUEST_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?i)\b(?:e-?mail|address)\b"));
static OWNER_RE: LazyLock<Re> = LazyLock::new(|| {
    Re::new(concat!(
        r"(?i)\bowner\b|\baccountable\b|\bwho(?:ever)?\s+(?:\w+\s+){0,2}(?:holds|has|runs|owns|is running)\b",
        r"|\bwhoever\s+\w+\s+it\b|\bholds it\b|\bhas it\b"
    ))
});
static EMAIL_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[\w.+-]+@[\w-]+\.[\w.-]+"));

/// The request asks for the email/address of whoever holds or owns something (the person is not named).
pub fn asks_owner_contact(question: &str) -> bool {
    !question.is_empty() && EMAIL_REQUEST_RE.is_match(question) && OWNER_RE.is_match(question)
}

/// Every recorded name form of the persons a text names by a full name or alias (alias index).
///
/// A Python set: it only feeds `email_records` (`names_any`, an `any`), so its order never reaches output.
pub fn people_named(mem: &UserMemory, text: &str) -> BTreeSet<String> {
    let mut forms = BTreeSet::new();
    for (alias, people) in &mem.alias_people {
        let hit_alias = names_form(alias, text);
        for full in people {
            if hit_alias || names_form(full, text) {
                forms.insert(alias.clone());
                forms.insert(full.clone());
            }
        }
    }
    forms
}

/// Records that state an email address and name one of the given name forms, oldest first.
pub fn email_records(mem: &UserMemory, forms: &BTreeSet<String>) -> Vec<Pair> {
    // Pure tests, cheapest first: an address has an '@', and the name test has a literal prefilter.
    let out = mem
        .pairs
        .values()
        .filter(|p| p.prompt.contains('@') && names_any(&p.prompt, forms) && EMAIL_RE.is_match(&p.prompt))
        .cloned()
        .collect();
    oldest_first(out)
}

/// The record texts of a memory context (empty when there is none or it is not a record payload).
///
/// Where the Python raises (a payload that is not an object, or `memories` a truthy number/bool), this returns "".
pub fn context_texts(context: Option<&str>) -> String {
    let Some(payload) = load_payload(context) else {
        return String::new();
    };
    let Some(obj) = payload.as_object() else {
        return String::new();
    };
    match obj.get("memories") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_object)
            .map(|it| match it.get("text") {
                Some(v) if crate::store::truthy(v) => py::str_value(v),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        // A string or an object iterates as strings: none is a dict.
        _ => String::new(),
    }
}

/// `list(payload.get("memories") or [])` when every element is a dict; None where the Python raises (a later
/// `it.get(...)` sort key on a non-dict element, or `list()` of a number).
fn memory_items(payload: &Map<String, Value>) -> Option<Vec<Value>> {
    match payload.get("memories") {
        None => Some(Vec::new()),
        Some(v) if !crate::store::truthy(v) => Some(Vec::new()),
        Some(Value::Array(items)) if items.iter().all(Value::is_object) => Some(items.clone()),
        _ => None,
    }
}

/// `items` plus `records` not yet present (by `str(it.get("pairID"))`), sorted by `str(it.get("timestamp", ""))`.
fn merged_items(mut items: Vec<Value>, records: &[Pair]) -> Vec<Value> {
    let have: HashSet<String> = items
        .iter()
        .filter_map(Value::as_object)
        .map(|it| py::str_value(it.get("pairID").unwrap_or(&Value::Null)))
        .collect();
    for p in records {
        if !have.contains(&p.pair_id) {
            items.push(json!({"pairID": p.pair_id, "timestamp": fmt_ts(&p.ts), "text": render_pair(p)}));
        }
    }
    let key = |it: &Value| -> String {
        it.as_object()
            .and_then(|o| o.get("timestamp"))
            .map(py::str_value)
            .unwrap_or_default()
    };
    let mut keyed: Vec<(String, Value)> = items.into_iter().map(|it| (key(&it), it)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, it)| it).collect()
}

/// The memory context with `records` added in full (chronological, no duplicates).
///
/// Where the Python raises (a payload that is not an object, a non-dict memory item), the context is unchanged.
pub fn with_records(context: Option<String>, prefix: &str, records: &[Pair]) -> Option<String> {
    let Some(mut payload) = load_payload(context.as_deref()) else {
        return context;
    };
    let Some(obj) = payload.as_object_mut() else {
        return context;
    };
    let Some(items) = memory_items(obj) else { return context };
    obj.insert("memories".into(), Value::Array(merged_items(items, records)));
    Some(dump_context(prefix, &payload))
}

// ---- contact requests about someone identified through a nicknamed person ("Lucky's colleague")

/// {nickname: full name} for recorded nicknames (with exactly one recorded full name) that the request uses
/// in possessive form ("Lucky's") while asking for an email or address; empty for any other request.
///
/// In `mem.alias_people` insertion order, as the Python dict.
pub fn possessive_nicknames(mem: &UserMemory, question: &str) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    if question.is_empty() || !EMAIL_REQUEST_RE.is_match(question) {
        return out;
    }
    for (alias, people) in &mem.alias_people {
        if people.len() == 1
            && question.contains(alias.as_str())
            && cached_re(format!(r"\b{}['’]s\b", escape(alias))).is_match(question)
        {
            if let Some(full) = people.iter().next() {
                out.insert(alias.clone(), full.clone());
            }
        }
    }
    out
}

/// Records declaring the full name behind each nickname (store.alias_declaration), oldest first.
pub fn identity_records(mem: &UserMemory, nicknames: &IndexMap<String, String>) -> Vec<Pair> {
    let out = mem
        .pairs
        .values()
        // A declaration of one of the nicknames quotes it: prompts without any of them are skipped before the
        // (costly) declaration parse.
        .filter(|p| nicknames.keys().any(|n| p.prompt.contains(n.as_str())))
        .filter(|p| alias_declaration(&p.prompt).is_some_and(|d| nicknames.contains_key(&d.1)))
        .cloned()
        .collect();
    oldest_first(out)
}

/// The publicly documented grounded-decline form for this request class, with the records' own name forms:
/// if nothing is recorded for the person asked about, say who the related person is, without handing out that
/// person's own contact details.
pub fn related_person_note(named: &IndexMap<String, String>) -> String {
    let mut items: Vec<(&String, &String)> = named.iter().collect();
    items.sort();
    let who = items
        .iter()
        .map(|(alias, full)| format!("{alias} is {full}"))
        .collect::<Vec<_>>()
        .join("; ");
    let mut keys: Vec<&String> = named.keys().collect();
    keys.sort();
    let nick = keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" / ");
    format!(
        "The request identifies the person it asks about through {nick} (in the records, {who}). \
         If the records hold no such contact detail for the person asked about, say so and say who {nick} is \
         in the records (full name and how the records describe them); do not quote {nick}'s own email or \
         address, which belongs to them, not to the person asked about."
    )
}

/// The memory context with `records` added in full (chronological, no duplicates) and `note` appended to its
/// note; a new context when there was none.
///
/// The Python always returns a str: this always returns Some. Where the Python raises (a payload that parses but
/// is not an object, a non-dict memory item), the context is returned unchanged.
pub fn with_records_and_note(context: Option<String>, prefix: &str, records: &[Pair], note: &str) -> Option<String> {
    let mut payload = json!({"note": "", "memories": []});
    if let Some(parsed) = load_payload(context.as_deref()) {
        payload = parsed;
    }
    let Some(obj) = payload.as_object_mut() else {
        return context;
    };
    let Some(items) = memory_items(obj) else { return context };
    obj.insert("memories".into(), Value::Array(merged_items(items, records)));
    let full_note = appended_note(obj, note);
    obj.insert("note".into(), Value::String(full_note));
    Some(dump_context(prefix, &payload))
}

/// For a handle request whose channel records include the user's removal instruction: the removal rule plus
/// the grounded-decline form with the records' own name forms of the person asked about; empty otherwise.
pub fn removed_handle_note(mem: &UserMemory, question: &str) -> String {
    let records = channel_records(mem, question);
    if !has_removal_instruction(&records) {
        return String::new();
    }
    let low = question.to_lowercase();
    let mut known: BTreeSet<String> = BTreeSet::new();
    for (alias, people) in &mem.alias_people {
        for full in people {
            let full_low = full.to_lowercase();
            if low.contains(full_low.as_str()) && mem.aliases.get(&full_low).map_or(0, |a| a.len()) == 1 {
                known.insert(format!("{full} goes by {alias}"));
            }
        }
    }
    let who = if known.is_empty() {
        String::new()
    } else {
        format!(
            " (in the records, {})",
            known.into_iter().collect::<Vec<_>>().join("; ")
        )
    };
    format!(
        "{REMOVAL_NOTE} If you decline, say who the person asked about is in the records{who}: the name they go \
         by and how the records describe them, without the removed handle."
    )
}
