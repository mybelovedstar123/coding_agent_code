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
