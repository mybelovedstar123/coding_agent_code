//! In-process memory store: per-user pairs, subjects and links, with hybrid (dense + BM25) retrieval.
//!
//! Everything lives in memory; the scored container runs on a read-only root filesystem and the store only has to
//! survive one scoring run.

use crate::embed::{dot, Embed, EmbedError};
use crate::excerpt::{expand_terms, Idf};
use crate::py;
use crate::pyre::{escape, Re};
use crate::text::tokenize;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use indexmap::IndexMap;
use serde_json::Value;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::RwLock;
use tokio::time::Instant;

/// A record without a usable timestamp sorts as the oldest, never as "now" (D14: the wall clock reached prompts).
pub fn epoch() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("epoch")
}

/// `datetime.fromisoformat(value.replace("Z", "+00:00"))` (Python 3.12 grammar for the forms records use), naive
/// times taken as UTC; anything unusable is the epoch.
pub fn parse_ts(value: Option<&Value>) -> DateTime<Utc> {
    let text = match value {
        None | Some(Value::Null) => return epoch(),
        Some(Value::String(s)) if s.is_empty() => return epoch(),
        Some(Value::Bool(false)) => return epoch(),
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => return epoch(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => py::str_value(other),
    };
    parse_iso(&text.replace('Z', "+00:00")).unwrap_or_else(epoch)
}
