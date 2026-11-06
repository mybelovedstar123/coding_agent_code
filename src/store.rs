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

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 10 {
        return None;
    }
    let date_str: String = chars[..10].iter().collect();
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    if chars.len() == 10 {
        return Some(Utc.from_utc_datetime(&date.and_time(NaiveTime::MIN)));
    }
    // Any single character separates the date and the time.
    let rest: String = chars[11..].iter().collect();
    let (time_part, offset) = split_offset(&rest)?;
    let time = parse_time(&time_part)?;
    let naive = NaiveDateTime::new(date, time);
    Some(match offset {
        None => Utc.from_utc_datetime(&naive),
        Some(off) => off.from_local_datetime(&naive).single()?.with_timezone(&Utc),
    })
}

fn split_offset(s: &str) -> Option<(String, Option<FixedOffset>)> {
    match s.find(['+', '-']) {
        None => Some((s.to_string(), None)),
        Some(i) => {
            let sign = if &s[i..i + 1] == "-" { -1 } else { 1 };
            let off = &s[i + 1..];
            let digits: String = off.chars().filter(|c| *c != ':').collect();
            let (h, m, sec) = match digits.len() {
                2 => (digits[..2].parse::<i32>().ok()?, 0, 0),
                4 => (digits[..2].parse().ok()?, digits[2..4].parse().ok()?, 0),
                6 => (
                    digits[..2].parse().ok()?,
                    digits[2..4].parse().ok()?,
                    digits[4..6].parse().ok()?,
                ),
                _ => return None,
            };
            Some((
                s[..i].to_string(),
                Some(FixedOffset::east_opt(sign * (h * 3600 + m * 60 + sec))?),
            ))
        }
    }
}

fn parse_time(s: &str) -> Option<NaiveTime> {
    let (main, frac) = match s.split_once(['.', ',']) {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    let parts: Vec<&str> = main.split(':').collect();
    let num = |x: &str| -> Option<u32> {
        if x.len() == 2 && x.chars().all(|c| c.is_ascii_digit()) {
            x.parse().ok()
        } else {
            None
        }
    };
    let (h, m, sec) = match parts.as_slice() {
        [h] => (num(h)?, 0, 0),
        [h, m] => (num(h)?, num(m)?, 0),
        [h, m, sec] => (num(h)?, num(m)?, num(sec)?),
        _ => return None,
    };
    let micros = match frac {
        None => 0,
        Some(f) if !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()) => {
            let mut digits: String = f.chars().take(6).collect();
            while digits.len() < 6 {
                digits.push('0');
            }
            digits.parse().ok()?
        }
        Some(_) => return None,
    };
    NaiveTime::from_hms_micro_opt(h, m, sec, micros)
}

/// `ts.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")`.
pub fn fmt_ts(ts: &DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[derive(Debug, Clone)]
pub struct Pair {
    pub pair_id: String,
    pub session_id: String,
    pub ts: DateTime<Utc>,
    pub prompt: String,
    pub response: String,
    pub source: String,
    pub subjects: BTreeSet<String>,
}

impl Pair {
    /// `f"{prompt}\n{response}".strip()`.
    pub fn text(&self) -> String {
        py::strip(&format!("{}\n{}", self.prompt, self.response)).to_string()
    }
}

pub fn render_pair(pair: &Pair) -> String {
    let mut parts = Vec::new();
    let prompt = py::strip(&pair.prompt);
    if !prompt.is_empty() {
        parts.push(format!("User: {prompt}"));
    }
    let response = py::strip(&pair.response);
    if !response.is_empty() {
        parts.push(format!("Ditto: {response}"));
    }
    parts.join("\n")
}

/// A total order over records from their content only: timestamp, then text. Ids are re-randomised every run and
/// concurrent writes change insertion order, so neither may decide an order that reaches a prompt (D15).
pub fn content_key(pair: &Pair) -> (DateTime<Utc>, &str, &str) {
    (pair.ts, pair.prompt.as_str(), pair.response.as_str())
}

pub fn cmp_content(a: &Pair, b: &Pair) -> Ordering {
    content_key(a).cmp(&content_key(b))
}
