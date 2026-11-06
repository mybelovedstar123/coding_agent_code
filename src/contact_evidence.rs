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
