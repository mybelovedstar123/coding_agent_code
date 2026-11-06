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

#[derive(Debug, Clone)]
pub struct Subject {
    pub subject_id: String,
    pub text: String,
    pub description: String,
    pub pairs: BTreeSet<String>,
}

// Alias index (declared in PIPELINE.md, context construction / memory search). People are often recorded under a
// nickname in some records and under their full name in others. A record of the user's own data that writes exactly
// one quoted single name next to exactly one full name introduces that name as the person's alias ("Ollie" is what
// everyone calls Olivia Cantu). Searches that mention one form also search the other, so records about the same
// person are found under either name. Retrieval only: it selects evidence, it never writes or picks an answer.
static ALIAS_QUOTE_RE: LazyLock<Re> = LazyLock::new(|| Re::new("[\u{201c}\"]([A-Z][a-z]{1,15})[.,]?[\u{201d}\"]"));
static FULL_NAME_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\b([A-Z][a-z]+ [A-Z][a-z]+(?:[-'][A-Z]?[a-z]+)?)\b"));

/// (full name, alias) when the text quotes exactly one single name next to exactly one full name.
pub fn alias_declaration(text: &str) -> Option<(String, String)> {
    let quoted: BTreeSet<String> = ALIAS_QUOTE_RE.findall(text).into_iter().collect();
    let names: BTreeSet<String> = FULL_NAME_RE
        .findall(text)
        .into_iter()
        .filter(|n| !quoted.iter().any(|q| py::split_ws(n).contains(&q.as_str())))
        .collect();
    if quoted.len() == 1 && names.len() == 1 {
        return Some((names.into_iter().next()?, quoted.into_iter().next()?));
    }
    None
}

/// Data derived from one user's records (indexes built on first use), dropped whenever a record changes (D4).
#[derive(Default)]
pub struct Derived(HashMap<&'static str, Arc<dyn Any + Send + Sync>>);

impl Derived {
    pub fn get<T: Any + Send + Sync>(&self, key: &'static str) -> Option<Arc<T>> {
        self.0.get(key).cloned().and_then(|a| a.downcast::<T>().ok())
    }

    pub fn put<T: Any + Send + Sync>(&mut self, key: &'static str, value: Arc<T>) {
        self.0.insert(key, value);
    }
}

/// One user's memory graph. Mutations hold the write lock; reads take the read lock only for a computation.
pub struct UserMemory {
    pub user_id: String,
    pub pairs: IndexMap<String, Pair>,
    pub subjects: IndexMap<String, Subject>,
    // Dense index (row-aligned with `order`).
    order: Vec<String>,
    row: HashMap<String, usize>,
    vecs: Vec<Vec<f32>>,
    // Lexical index; a token's document frequency stays in `df` at zero once its last record changes (as a Python
    // Counter keeps the key), so the typo-tolerant vocabulary is the same.
    tf: IndexMap<String, IndexMap<String, usize>>,
    tlen: HashMap<String, usize>,
    pub df: IndexMap<String, i64>,
    // Alias index: full name (lower) -> aliases, alias -> full names (see alias_declaration).
    pub aliases: IndexMap<String, BTreeSet<String>>,
    pub alias_people: IndexMap<String, BTreeSet<String>>,
    // Subject vectors.
    subj_order: Vec<String>,
    subj_vecs: Vec<Vec<f32>>,
    pub derived: Mutex<Derived>,
}

impl UserMemory {
    pub fn new(user_id: &str) -> Self {
        UserMemory {
            user_id: user_id.to_string(),
            pairs: IndexMap::new(),
            subjects: IndexMap::new(),
            order: Vec::new(),
            row: HashMap::new(),
            vecs: Vec::new(),
            tf: IndexMap::new(),
            tlen: HashMap::new(),
            df: IndexMap::new(),
            aliases: IndexMap::new(),
            alias_people: IndexMap::new(),
            subj_order: Vec::new(),
            subj_vecs: Vec::new(),
            derived: Mutex::new(Derived::default()),
        }
    }

    /// The number of records in the lexical index (`len(self._tf)`).
    pub fn n_docs(&self) -> usize {
        self.tf.len()
    }

    pub fn idf(&self) -> Idf<'_> {
        Idf {
            df: &self.df,
            n: self.n_docs(),
        }
    }

    /// A derived index, built by `make` on first use after the records last changed.
    pub fn derived<T: Any + Send + Sync>(&self, key: &'static str, make: impl FnOnce() -> T) -> Arc<T> {
        if let Some(found) = self.derived.lock().expect("derived lock").get::<T>(key) {
            return found;
        }
        let built = Arc::new(make());
        self.derived.lock().expect("derived lock").put(key, built.clone());
        built
    }

    // ----------------------------------------------------------------- writes
    fn index_lexical(&mut self, pair: &Pair) {
        if let Some(old) = self.tf.get(&pair.pair_id) {
            for tok in old.keys() {
                *self.df.get_mut(tok).expect("indexed token") -= 1;
            }
        }
        let toks = tokenize(&pair.text());
        let mut tf: IndexMap<String, usize> = IndexMap::new();
        for t in &toks {
            *tf.entry(t.clone()).or_insert(0) += 1;
        }
        for tok in tf.keys() {
            *self.df.entry(tok.clone()).or_insert(0) += 1;
        }
        self.tlen.insert(pair.pair_id.clone(), toks.len().max(1));
        self.tf.insert(pair.pair_id.clone(), tf);
    }

    fn index_alias(&mut self, pair: &Pair) {
        if let Some((full, alias)) = alias_declaration(&pair.prompt) {
            self.aliases
                .entry(full.to_lowercase())
                .or_default()
                .insert(alias.clone());
            self.alias_people.entry(alias).or_default().insert(full);
        }
    }

    /// Extra search queries that name the other form of a person the query names.
    pub fn alias_queries(&self, query: &str) -> Vec<String> {
        let low = query.to_lowercase();
        let mut extra = Vec::new();
        for (full, aliases) in &self.aliases {
            if low.contains(full.as_str()) {
                extra.push(format!(
                    "{query} {}",
                    aliases.iter().cloned().collect::<Vec<_>>().join(" ")
                ));
            }
        }
        for (alias, people) in &self.alias_people {
            // Capitalized use only: "Happy" the name, not "happy".
            if Re::new(&format!(r"\b{}\b", escape(alias))).is_match(query) {
                extra.push(format!(
                    "{query} {}",
                    people.iter().cloned().collect::<Vec<_>>().join(" ")
                ));
            }
        }
        extra
    }

    pub fn upsert_pairs(&mut self, pairs: Vec<Pair>, vecs: Vec<Vec<f32>>) {
        *self.derived.lock().expect("derived lock") = Derived::default();
        for (mut pair, vec) in pairs.into_iter().zip(vecs) {
            if let Some(existing) = self.pairs.get(&pair.pair_id) {
                pair.subjects.extend(existing.subjects.iter().cloned());
            }
            self.index_lexical(&pair);
            self.index_alias(&pair);
            match self.row.get(&pair.pair_id) {
                None => {
                    self.row.insert(pair.pair_id.clone(), self.order.len());
                    self.order.push(pair.pair_id.clone());
                    self.vecs.push(vec);
                }
                Some(&r) => self.vecs[r] = vec,
            }
            self.pairs.insert(pair.pair_id.clone(), pair);
        }
    }

    pub fn upsert_subjects(&mut self, subjects: Vec<Subject>, vecs: Vec<Vec<f32>>) {
        for (mut subj, vec) in subjects.into_iter().zip(vecs) {
            if let Some(existing) = self.subjects.get(&subj.subject_id) {
                subj.pairs.extend(existing.pairs.iter().cloned());
                let idx = self
                    .subj_order
                    .iter()
                    .position(|s| *s == subj.subject_id)
                    .expect("indexed subject");
                self.subj_vecs[idx] = vec;
            } else {
                self.subj_order.push(subj.subject_id.clone());
                self.subj_vecs.push(vec);
            }
            self.subjects.insert(subj.subject_id.clone(), subj);
        }
    }

    pub fn link(&mut self, subject_id: &str, pair_id: &str) {
        if !self.subjects.contains_key(subject_id) || !self.pairs.contains_key(pair_id) {
            return;
        }
        self.subjects
            .get_mut(subject_id)
            .expect("checked")
            .pairs
            .insert(pair_id.to_string());
        self.pairs
            .get_mut(pair_id)
            .expect("checked")
            .subjects
            .insert(subject_id.to_string());
    }

    // ------------------------------------------------------------------ reads
    pub fn dense_scores(&self, qvec: &[f32], ids: Option<&[String]>) -> Vec<(String, f64)> {
        match ids {
            None => self
                .order
                .iter()
                .zip(&self.vecs)
                .map(|(pid, v)| (pid.clone(), dot(v, qvec) as f64))
                .collect(),
            Some(ids) => ids
                .iter()
                .filter_map(|i| {
                    self.row
                        .get(i)
                        .map(|&r| (self.order[r].clone(), dot(&self.vecs[r], qvec) as f64))
                })
                .collect(),
        }
    }

    /// BM25 with typo-tolerant term matching (one edit on 5+ char terms).
    pub fn bm25_scores(&self, query: &str, ids: Option<&[String]>) -> Vec<(String, f64)> {
        // Sorted: float sums must not follow set order (D16).
        let qtoks: Vec<String> = tokenize(query)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if qtoks.is_empty() || self.tf.is_empty() {
            return Vec::new();
        }
        let n = self.tf.len() as f64;
        let avg = self.tlen.values().sum::<usize>() as f64 / n;
        let (k1, b) = (1.2f64, 0.75f64);
        let df = &self.df;
        let expanded = expand_terms(
            &qtoks,
            || Box::new(df.keys().map(|k| k.as_str())),
            |t| df.contains_key(t),
        );
        let pool: Vec<&String> = match ids {
            Some(ids) => ids.iter().collect(),
            None => self.tf.keys().collect(),
        };
        let mut scores = Vec::new();
        for pid in pool {
            let Some(tf) = self.tf.get(pid) else { continue };
            let norm = k1 * (1.0 - b + b * self.tlen[pid] as f64 / avg);
            let mut s = 0.0f64;
            for matches in expanded.values() {
                let mut best = 0.0f64;
                for (tok, w) in matches {
                    let Some(&f) = tf.get(tok) else { continue };
                    if f == 0 {
                        continue;
                    }
                    let d = df[tok] as f64;
                    let idf = (1.0 + (n - d + 0.5) / (d + 0.5)).ln();
                    let f = f as f64;
                    best = best.max(w * idf * f * (k1 + 1.0) / (f + norm));
                }
                s += best;
            }
            if s > 0.0 {
                scores.push((pid.clone(), s));
            }
        }
        scores
    }

    pub fn subject_scores(&self, qvec: &[f32], query: &str) -> Vec<(String, f64)> {
        if self.subj_order.is_empty() {
            return Vec::new();
        }
        let qtoks: BTreeSet<String> = tokenize(query).into_iter().collect();
        let mut out: Vec<(String, f64)> = self
            .subj_order
            .iter()
            .zip(&self.subj_vecs)
            .map(|(sid, v)| {
                let subj = &self.subjects[sid];
                let name_toks: BTreeSet<String> = tokenize(&subj.text).into_iter().collect();
                let lex = qtoks.intersection(&name_toks).count() as f64 / name_toks.len().max(1) as f64;
                (sid.clone(), dot(v, qvec) as f64 + 0.3 * lex)
            })
            .collect();
        out.sort_by(|a, b| self.cmp_subject_score(a, b));
        out
    }

    fn cmp_subject_score(&self, a: &(String, f64), b: &(String, f64)) -> Ordering {
        let (sa, sb) = (&self.subjects[&a.0], &self.subjects[&b.0]);
        cmp_f64(-a.1, -b.1)
            .then_with(|| sa.text.cmp(&sb.text))
            .then_with(|| sa.description.cmp(&sb.description))
    }
}

/// Floats ordered as Python orders them in a sort key (no NaN arises here).
pub fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}
