//! Event histories in stored records -> the state they describe at an effective step.
//!
//! Records of an event history state their own rules (events apply by effective step, not document order; an
//! assignment replaces a field's value; add/remove change only the named member of a set; state persists until
//! changed). The events come as tables (JSON lists of objects, CSV, markdown tables with entity / field / value /
//! effective step / operation columns) or as sentences that quote the entity, field and value and name the step. This
//! module reads the events, applies those rules up to a step, and returns every entity's fields at that step, so the
//! deciding model sees the state the records describe next to the records themselves.
//!
//! Python semantics carried here (private helpers, see the report): `json.loads` (NaN/Infinity, arbitrary-precision
//! ints, the 4300-digit int limit), `int()` of a str/float, `str.splitlines()`, the `csv` module's reader state machine
//! with `DictReader`, and Python's `\s`/`\w`/`\b` classes in patterns (`py_pattern`).

use crate::py;
use crate::pyre::Re;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

pub const Q: &str = r#""([^"]+)""#;
pub static STEP_RE: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?i)\bstep\s+(\d+)\b"));
pub static SENT_SPLIT: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?<=[.;])\s+(?=[A-Z\[])|\n+"));
pub const MIN_EVENTS: usize = 10;
pub const EVENT_STATE_NOTE: &str = "event_state: a host-side reading of the event history in the records for the \
identifier in the request, replayed up to effective step {step} under the rules those records state (events by \
effective step, not document order; an assignment replaces the field's value; add/remove change only the named member \
of a set; state persists until changed), with every entity's fields at that step and sets as lists. It is an \
unverified candidate, not an answer, and can be wrong: check what you rely on against the records themselves, and \
where they differ, the records decide.";

static TAG_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"<[^>]+>"));
static NON_ALNUM_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"[^a-z0-9]+"));
static JSON_LIST_RE: LazyLock<PyRe> = LazyLock::new(|| PyRe::new(r"(?s)\[\s*\{.*?\}\s*\]"));
static QUOTED_RE: LazyLock<Re> = LazyLock::new(|| Re::new(Q));
static ADD_REMOVE_RE: LazyLock<PyRe> = LazyLock::new(|| {
    PyRe::new(&format!(
        r"(?i)\b(add|remove)\s+{Q}\s+(?:to|from)\s+the\s+{Q}\s+set\s+of\s+{Q}"
    ))
});
static DIGIT_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"^\d$"));

/// Python's `\w` class body (str patterns): `str.isalnum()` (letters and every numeric category) or `_`.
const PY_WORD: &str = r"\p{L}\p{N}_";

/// A Python `re` pattern rewritten where the engines' classes differ: `\s` also takes U+001C..U+001F, `\w` (and so
/// `\b`) is letters, numbers and `_` only (no combining marks, no joiners, no other connector punctuation).
pub(crate) fn py_pattern(p: &str) -> String {
    let word = format!("[{PY_WORD}]");
    let boundary = format!("(?:(?<!{word})(?={word})|(?<={word})(?!{word}))");
    let mut out = String::with_capacity(p.len() * 2);
    let mut in_class = false;
    let mut chars = p.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(n) = chars.next() else {
                out.push(c);
                break;
            };
            match (n, in_class) {
                ('w', false) => out.push_str(&word),
                ('w', true) => out.push_str(PY_WORD),
                ('s', false) => out.push_str(r"[\s\x1c-\x1f]"),
                ('s', true) => out.push_str(r"\s\x1c-\x1f"),
                ('b', false) => out.push_str(&boundary),
                _ => {
                    out.push('\\');
                    out.push(n);
                }
            }
            continue;
        }
        out.push(c);
        if in_class {
            if c == ']' {
                in_class = false;
            }
        } else if c == '[' {
            in_class = true;
            if chars.peek() == Some(&'^') {
                out.push(chars.next().unwrap_or('^'));
            }
            if chars.peek() == Some(&']') {
                out.push_str("\\]");
                chars.next();
            }
        }
    }
    out
}

/// Whether Rust's and Python's `\w` and `\s` agree on a character: ASCII other than U+001C..U+001F, Latin-1 other
/// than the superscripts and fractions (category No), Latin Extended-A/B, and General Punctuation other than the
/// joiners and the connector punctuation.
fn classes_agree(c: char) -> bool {
    match c as u32 {
        0x1c..=0x1f => false,
        0..=0x7f => true,
        0xb2 | 0xb3 | 0xb9 | 0xbc..=0xbe => false,
        0xa0..=0x24f => true,
        0x200c | 0x200d | 0x203f | 0x2040 | 0x2054 => false,
        0x2000..=0x206f => true,
        _ => false,
    }
}

/// A Python pattern compiled twice: as written (Rust's classes) and through `py_pattern` (Python's classes). Text
/// whose every character the two class sets treat alike runs on the first, which has no lookaround for `` and so
/// stays on the fast engine; any other text runs on the second. Both give Python's result.
pub struct PyRe {
    fast: Re,
    exact: Re,
}

impl PyRe {
    pub fn new(pattern: &str) -> Self {
        PyRe {
            fast: Re::new(pattern),
            exact: Re::new(&py_pattern(pattern)),
        }
    }

    /// The compiled pattern to run on `text`.
    pub fn on(&self, text: &str) -> &Re {
        if text.chars().all(classes_agree) {
            &self.fast
        } else {
            &self.exact
        }
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.on(text).is_match(text)
    }

    pub fn captures<'a>(&self, text: &'a str) -> Option<fancy_regex::Captures<'a>> {
        self.on(text).captures(text)
    }

    pub fn captures_iter<'a>(&self, text: &'a str) -> Vec<fancy_regex::Captures<'a>> {
        self.on(text).captures_iter(text)
    }

    pub fn match_start<'a>(&self, text: &'a str) -> Option<fancy_regex::Captures<'a>> {
        self.on(text).match_start(text)
    }

    pub fn find_iter<'a>(&self, text: &'a str) -> Vec<(usize, usize, &'a str)> {
        self.on(text).find_iter(text)
    }

    pub fn findall(&self, text: &str) -> Vec<String> {
        self.on(text).findall(text)
    }

    pub fn split<'a>(&self, text: &'a str) -> Vec<&'a str> {
        self.on(text).split(text)
    }
}

// ------------------------------------------------------------------------------------------- Python value semantics

/// A Python int (arbitrary precision): sign and decimal digits without leading zeros ("0" for zero).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyInt {
    neg: bool,
    digits: String,
}

/// CPython's default `sys.int_info.default_max_str_digits`.
const INT_MAX_STR_DIGITS: usize = 4300;

impl PyInt {
    fn new(neg: bool, digits: &str) -> PyInt {
        let d = digits.trim_start_matches('0');
        if d.is_empty() {
            PyInt {
                neg: false,
                digits: "0".into(),
            }
        } else {
            PyInt { neg, digits: d.into() }
        }
    }

    pub fn from_i64(v: i64) -> PyInt {
        PyInt::new(v < 0, &v.unsigned_abs().to_string())
    }

    /// `int(x)` for a finite float (truncation toward zero, exact).
    fn from_f64(x: f64) -> Option<PyInt> {
        if !x.is_finite() {
            return None;
        }
        let t = x.trunc();
        Some(PyInt::new(t < 0.0, &format!("{:.0}", t.abs())))
    }

    /// `int(s)` for a str (base 10): Unicode decimal digits and whitespace, one sign, single underscores between
    /// digits; None where Python raises ValueError.
    pub fn parse(s: &str) -> Option<PyInt> {
        // _PyUnicode_TransformDecimalAndSpaceToASCII, then PyLong_FromString.
        let mapped: String = s
            .chars()
            .map(|c| {
                if py::is_space(c) {
                    ' '
                } else if let Some(d) = decimal_value(c) {
                    char::from(b'0' + d as u8)
                } else if c.is_ascii() {
                    c
                } else {
                    '?'
                }
            })
            .collect();
        let t = mapped.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'));
        let (neg, body) = match t.as_bytes().first() {
            Some(b'-') => (true, &t[1..]),
            Some(b'+') => (false, &t[1..]),
            _ => (false, t),
        };
        if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
            return None;
        }
        if !body.chars().all(|c| c.is_ascii_digit() || c == '_') {
            return None;
        }
        let digits: String = body.chars().filter(|&c| c != '_').collect();
        if digits.len() > INT_MAX_STR_DIGITS {
            return None;
        }
        Some(PyInt::new(neg, &digits))
    }
}

impl fmt::Display for PyInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.neg {
            f.write_str("-")?;
        }
        f.write_str(&self.digits)
    }
}

impl Ord for PyInt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.neg, other.neg) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (neg, _) => {
                let mag = self
                    .digits
                    .len()
                    .cmp(&other.digits.len())
                    .then_with(|| self.digits.cmp(&other.digits));
                if neg {
                    mag.reverse()
                } else {
                    mag
                }
            }
        }
    }
}

impl PartialOrd for PyInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// `unicodedata.decimal(c)`: the value of a Unicode decimal digit (category Nd, which comes in runs of ten).
fn decimal_value(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        return Some(c as u32 - '0' as u32);
    }
    if c.is_ascii() || !DIGIT_RE.is_match(c.encode_utf8(&mut [0; 4])) {
        return None;
    }
    let mut n = 0u32;
    let mut cp = c as u32;
    while cp > 0 {
        cp -= 1;
        match char::from_u32(cp) {
            Some(p) if DIGIT_RE.is_match(p.encode_utf8(&mut [0; 4])) => n += 1,
            _ => break,
        }
    }
    Some(n % 10)
}

/// A value as Python's `json.loads` makes it.
#[derive(Clone, Debug)]
pub enum PyVal {
    None,
    Bool(bool),
    Int(PyInt),
    Float(f64),
    Str(String),
    List(Vec<PyVal>),
    Dict(IndexMap<String, PyVal>),
}

impl PyVal {
    /// `repr(v)`.
    fn repr(&self) -> String {
        match self {
            PyVal::Str(s) => py::repr_str(s),
            PyVal::List(items) => format!("[{}]", items.iter().map(PyVal::repr).collect::<Vec<_>>().join(", ")),
            PyVal::Dict(m) => format!(
                "{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{}: {}", py::repr_str(k), v.repr()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => other.str(),
        }
    }

    /// `str(v)`.
    fn str(&self) -> String {
        match self {
            PyVal::None => "None".into(),
            PyVal::Bool(b) => if *b { "True" } else { "False" }.into(),
            PyVal::Int(i) => i.to_string(),
            PyVal::Float(x) => py::float_repr(*x),
            PyVal::Str(s) => s.clone(),
            other => other.repr(),
        }
    }

    /// `int(v)`; None where Python raises TypeError, ValueError or OverflowError.
    fn int(&self) -> Option<PyInt> {
        match self {
            PyVal::Bool(b) => Some(PyInt::from_i64(i64::from(*b))),
            PyVal::Int(i) => Some(i.clone()),
            PyVal::Float(x) => PyInt::from_f64(*x),
            PyVal::Str(s) => PyInt::parse(s),
            _ => None,
        }
    }
}

/// Python's `json.loads(s)` (the C scanner, strict): None where it raises ValueError. A lone surrogate escape, which
/// Python keeps in the str, becomes U+FFFD (a Rust string cannot hold it).
pub fn py_json_loads(s: &str) -> Option<PyVal> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = JsonParser { s: &chars, depth: 0 };
    let i = p.ws(0);
    let (v, end) = p.value(i)?;
    let end = p.ws(end);
    (end == chars.len()).then_some(v)
}

struct JsonParser<'a> {
    s: &'a [char],
    depth: usize,
}

impl JsonParser<'_> {
    fn at(&self, i: usize) -> Option<char> {
        self.s.get(i).copied()
    }

    fn ws(&self, mut i: usize) -> usize {
        while matches!(self.at(i), Some(' ' | '\t' | '\n' | '\r')) {
            i += 1;
        }
        i
    }

    fn lit(&self, i: usize, word: &str) -> bool {
        word.chars().enumerate().all(|(k, c)| self.at(i + k) == Some(c))
    }

    fn value(&mut self, i: usize) -> Option<(PyVal, usize)> {
        match self.at(i)? {
            '"' => self.string(i + 1).map(|(s, e)| (PyVal::Str(s), e)),
            '{' => self.nested(i, true),
            '[' => self.nested(i, false),
            'n' if self.lit(i, "null") => Some((PyVal::None, i + 4)),
            't' if self.lit(i, "true") => Some((PyVal::Bool(true), i + 4)),
            'f' if self.lit(i, "false") => Some((PyVal::Bool(false), i + 5)),
            'N' if self.lit(i, "NaN") => Some((PyVal::Float(f64::NAN), i + 3)),
            'I' if self.lit(i, "Infinity") => Some((PyVal::Float(f64::INFINITY), i + 8)),
            '-' if self.lit(i, "-Infinity") => Some((PyVal::Float(f64::NEG_INFINITY), i + 9)),
            _ => self.number(i),
        }
    }

    fn nested(&mut self, i: usize, object: bool) -> Option<(PyVal, usize)> {
        // CPython's recursion limit (1000 frames) is far deeper than any record nests; bound it the same way.
        self.depth += 1;
        if self.depth > 990 {
            return None;
        }
        let out = if object { self.object(i + 1) } else { self.array(i + 1) };
        self.depth -= 1;
        out
    }

    fn object(&mut self, i: usize) -> Option<(PyVal, usize)> {
        let mut m = IndexMap::new();
        let mut i = self.ws(i);
        if self.at(i) == Some('}') {
            return Some((PyVal::Dict(m), i + 1));
        }
        loop {
            if self.at(i) != Some('"') {
                return None;
            }
            let (k, e) = self.string(i + 1)?;
            i = self.ws(e);
            if self.at(i) != Some(':') {
                return None;
            }
            i = self.ws(i + 1);
            let (v, e) = self.value(i)?;
            m.insert(k, v);
            i = self.ws(e);
            match self.at(i)? {
                '}' => return Some((PyVal::Dict(m), i + 1)),
                ',' => i = self.ws(i + 1),
                _ => return None,
            }
        }
    }

    fn array(&mut self, i: usize) -> Option<(PyVal, usize)> {
        let mut items = Vec::new();
        let mut i = self.ws(i);
        if self.at(i) == Some(']') {
            return Some((PyVal::List(items), i + 1));
        }
        loop {
            let (v, e) = self.value(i)?;
            items.push(v);
            i = self.ws(e);
            match self.at(i)? {
                ']' => return Some((PyVal::List(items), i + 1)),
                ',' => i = self.ws(i + 1),
                _ => return None,
            }
        }
    }

    fn hex4(&self, i: usize) -> Option<u32> {
        let mut v = 0;
        for k in 0..4 {
            v = v * 16 + self.at(i + k)?.to_digit(16)?;
        }
        Some(v)
    }

    fn string(&self, mut i: usize) -> Option<(String, usize)> {
        let mut out = String::new();
        loop {
            let c = self.at(i)?;
            match c {
                '"' => return Some((out, i + 1)),
                '\\' => {
                    let e = self.at(i + 1)?;
                    i += 2;
                    match e {
                        '"' | '\\' | '/' => out.push(e),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let u = self.hex4(i)?;
                            i += 4;
                            if (0xd800..0xdc00).contains(&u) && self.at(i) == Some('\\') && self.at(i + 1) == Some('u')
                            {
                                if let Some(lo) = self.hex4(i + 2).filter(|lo| (0xdc00..0xe000).contains(lo)) {
                                    let cp = 0x10000 + ((u - 0xd800) << 10) + (lo - 0xdc00);
                                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                                    i += 6;
                                    continue;
                                }
                            }
                            out.push(char::from_u32(u).unwrap_or('\u{fffd}'));
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => {
                    out.push(c);
                    i += 1;
                }
            }
        }
    }

    fn number(&self, start: usize) -> Option<(PyVal, usize)> {
        let digit = |i: usize| self.at(i).is_some_and(|c| c.is_ascii_digit());
        let mut i = start;
        if self.at(i) == Some('-') {
            i += 1;
        }
        match self.at(i)? {
            '1'..='9' => {
                i += 1;
                while digit(i) {
                    i += 1;
                }
            }
            '0' => i += 1,
            _ => return None,
        }
        let mut is_float = false;
        if self.at(i) == Some('.') && digit(i + 1) {
            is_float = true;
            i += 2;
            while digit(i) {
                i += 1;
            }
        }
        if matches!(self.at(i), Some('e' | 'E')) {
            let e_start = i;
            i += 1;
            if matches!(self.at(i), Some('-' | '+')) {
                i += 1;
            }
            while digit(i) {
                i += 1;
            }
            if digit(i - 1) {
                is_float = true;
            } else {
                i = e_start;
            }
        }
        let text: String = self.s[start..i].iter().collect();
        if is_float {
            Some((PyVal::Float(text.parse().ok()?), i))
        } else {
            Some((PyVal::Int(PyInt::parse(&text)?), i))
        }
    }
}

/// `s.splitlines()`.
pub(crate) fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let boundary = matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if boundary {
            out.push(&s[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' && it.peek().map(|&(_, n)| n) == Some('\n') {
                it.next();
                next += 1;
            }
            start = next;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

// ------------------------------------------------------------------------------------------------------ csv module

#[derive(Clone, Copy, PartialEq, Eq)]
enum CsvState {
    StartRecord,
    StartField,
    InField,
    InQuotedField,
    QuoteInQuotedField,
    EatCrnl,
}

/// `csv.field_size_limit()` default.
const CSV_FIELD_LIMIT: usize = 128 * 1024;

/// `list(csv.reader(io.StringIO(text)))` with the excel dialect; Err where the reader raises `csv.Error`.
fn csv_rows(text: &str) -> Result<Vec<Vec<String>>, ()> {
    // io.StringIO iterates lines split after each "\n", the newline kept.
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, _) in text.match_indices('\n') {
        lines.push(&text[start..=i]);
        start = i + 1;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    let mut lines = lines.into_iter();
    let mut out = Vec::new();
    loop {
        // Reader_iternext: parse_reset, then lines until the record ends.
        let mut r = CsvReader {
            state: CsvState::StartRecord,
            fields: Vec::new(),
            field: String::new(),
            field_len: 0,
        };
        loop {
            let Some(line) = lines.next() else {
                if r.field_len != 0 || r.state == CsvState::InQuotedField {
                    r.save_field();
                    out.push(r.fields);
                }
                return Ok(out);
            };
            for c in line.chars() {
                r.process_char(Some(c))?;
            }
            r.process_char(None)?;
            if r.state == CsvState::StartRecord {
                break;
            }
        }
        out.push(r.fields);
    }
}

struct CsvReader {
    state: CsvState,
    fields: Vec<String>,
    field: String,
    field_len: usize,
}

impl CsvReader {
    fn add_char(&mut self, c: char) -> Result<(), ()> {
        if self.field_len >= CSV_FIELD_LIMIT {
            return Err(());
        }
        self.field.push(c);
        self.field_len += 1;
        Ok(())
    }

    fn save_field(&mut self) {
        self.fields.push(std::mem::take(&mut self.field));
        self.field_len = 0;
    }

    /// `parse_process_char` of Modules/_csv.c for the excel dialect (`None` is EOL).
    fn process_char(&mut self, c: Option<char>) -> Result<(), ()> {
        let newline = matches!(c, Some('\n' | '\r'));
        let line_end = if c.is_none() {
            CsvState::StartRecord
        } else {
            CsvState::EatCrnl
        };
        if self.state == CsvState::StartRecord {
            if c.is_none() {
                return Ok(());
            }
            if newline {
                self.state = CsvState::EatCrnl;
                return Ok(());
            }
            self.state = CsvState::StartField;
        }
        match (self.state, c) {
            (CsvState::StartRecord, _) => {}
            (CsvState::StartField | CsvState::InField | CsvState::QuoteInQuotedField, _) if newline || c.is_none() => {
                self.save_field();
                self.state = line_end;
            }
            (CsvState::StartField, Some('"')) => self.state = CsvState::InQuotedField,
            (CsvState::StartField | CsvState::InField | CsvState::QuoteInQuotedField, Some(',')) => {
                self.save_field();
                self.state = CsvState::StartField;
            }
            (CsvState::StartField | CsvState::InField, Some(ch)) => {
                self.add_char(ch)?;
                self.state = CsvState::InField;
            }
            (CsvState::InQuotedField, None) => {}
            (CsvState::InQuotedField, Some('"')) => self.state = CsvState::QuoteInQuotedField,
            (CsvState::InQuotedField, Some(ch)) => self.add_char(ch)?,
            (CsvState::QuoteInQuotedField, Some('"')) => {
                self.add_char('"')?;
                self.state = CsvState::InQuotedField;
            }
            // strict=False: a character after the closing quote joins the field.
            (CsvState::QuoteInQuotedField, Some(ch)) => {
                self.add_char(ch)?;
                self.state = CsvState::InField;
            }
            (CsvState::StartField | CsvState::InField | CsvState::QuoteInQuotedField, None) => {}
            (CsvState::EatCrnl, None) => self.state = CsvState::StartRecord,
            (CsvState::EatCrnl, Some(_)) if newline => {}
            // "new-line character seen in unquoted field"
            (CsvState::EatCrnl, Some(_)) => return Err(()),
        }
        Ok(())
    }
}

/// `list(csv.DictReader(io.StringIO(text)))` with the rest key (None) left out; a missing value is `PyVal::None`.
fn csv_dicts(text: &str) -> Result<Vec<PyVal>, ()> {
    let mut rows = csv_rows(text)?.into_iter();
    let Some(fieldnames) = rows.next() else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for row in rows {
        if row.is_empty() {
            continue;
        }
        let mut d: IndexMap<String, PyVal> = IndexMap::new();
        for (k, v) in fieldnames.iter().zip(row.iter()) {
            d.insert(k.clone(), PyVal::Str(v.clone()));
        }
        if fieldnames.len() > row.len() {
            for k in &fieldnames[row.len()..] {
                d.insert(k.clone(), PyVal::None);
            }
        }
        out.push(PyVal::Dict(d));
    }
    Ok(out)
}

// -------------------------------------------------------------------------------------------------------- the module

/// (step, operation, entity, field, value).
pub type Event = (PyInt, String, String, String, String);

pub fn _norm_key(k: &str) -> String {
    let low = py::strip(&TAG_RE.sub(k, "")).to_lowercase();
    py::strip_chars(&NON_ALNUM_RE.sub(&low, "_"), "_").to_string()
}

pub fn _cell(v: &str) -> String {
    let s = TAG_RE.sub(v, "");
    py::strip(py::strip_chars(py::strip(&s), "`")).to_string()
}

pub fn _event(d: &PyVal) -> Option<Event> {
    let PyVal::Dict(items) = d else {
        return None;
    };
    let mut m: IndexMap<String, PyVal> = IndexMap::new();
    for (k, v) in items {
        let v = match v {
            PyVal::None => continue,
            PyVal::Bool(_) | PyVal::Int(_) | PyVal::Float(_) => v.clone(),
            other => PyVal::Str(_cell(&other.str())),
        };
        m.insert(_norm_key(k), v);
    }
    if !["entity", "field", "value", "effective_step", "operation"]
        .iter()
        .all(|k| m.contains_key(*k))
    {
        return None;
    }
    let step = m["effective_step"].int()?;
    Some((
        step,
        m["operation"].str().to_lowercase(),
        m["entity"].str(),
        m["field"].str(),
        m["value"].str(),
    ))
}

pub fn from_json(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    for (_, _, m) in JSON_LIST_RE.find_iter(text) {
        let Some(rows) = py_json_loads(m) else {
            continue;
        };
        if let PyVal::List(rows) = rows {
            out.extend(rows.iter().filter_map(_event));
        }
    }
    out
}

/// None where Python's csv reader raises (a field over `csv.field_size_limit()`), which Python does not catch.
fn try_from_csv(text: &str) -> Option<Vec<Event>> {
    let mut out = Vec::new();
    let lines = splitlines(text);
    for (i, line) in lines.iter().enumerate() {
        if line.contains("effective_step") && line.contains(',') {
            let mut block = vec![*line];
            for nxt in &lines[i + 1..] {
                if !nxt.contains(',') {
                    break;
                }
                block.push(nxt);
            }
            let rows = csv_dicts(&block.join("\n")).ok()?;
            out.extend(rows.iter().filter_map(_event));
        }
    }
    Some(out)
}

pub fn from_csv(text: &str) -> Vec<Event> {
    try_from_csv(text).unwrap_or_default()
}

pub fn from_markdown(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    let mut header: Option<Vec<String>> = None;
    for line in splitlines(text) {
        if !py::strip(line).starts_with('|') {
            header = None;
            continue;
        }
        let cells: Vec<String> = py::strip_chars(py::strip(line), "|")
            .split('|')
            .map(|c| py::strip(c).to_string())
            .collect();
        if cells.iter().all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
            continue;
        }
        let Some(h) = &header else {
            header = Some(cells);
            continue;
        };
        let mut d = IndexMap::new();
        for (k, v) in h.iter().zip(cells) {
            d.insert(k.clone(), PyVal::Str(v));
        }
        if let Some(e) = _event(&PyVal::Dict(d)) {
            out.push(e);
        }
    }
    out
}

/// Sentences that quote an entity, a field and a value and name the effective step: "add/remove "X" to/from the
/// "F" set of "E"", or an assignment ("the value of "F" becomes ...", "the assignment of "V" to field "F" of "E"",
/// ""E" has "F" set to ...") that replaces or supersedes the earlier value.
pub fn from_sentences(text: &str) -> Vec<Event> {
    let mut out = Vec::new();
    for s in SENT_SPLIT.split(text) {
        let Some(st) = STEP_RE.captures(s) else {
            continue;
        };
        let quoted = QUOTED_RE.captures_iter(s);
        if quoted.len() != 3 {
            continue;
        }
        // int() of a \d+ match only fails past the 4300-digit limit, where Python raises.
        let Some(step) = PyInt::parse(&st[1]) else {
            continue;
        };
        if let Some(m) = ADD_REMOVE_RE.captures(s) {
            out.push((
                step,
                m[1].to_lowercase(),
                m[4].to_string(),
                m[3].to_string(),
                m[2].to_string(),
            ));
            continue;
        }
        let (mut entity, mut field, mut value): (Option<&str>, Option<&str>, Option<&str>) = (None, None, None);
        for q in &quoted {
            let (whole, inner) = (q.get(0).expect("group 0"), q.get(1).map_or("", |g| g.as_str()));
            let before = py::rstrip(&s[..whole.start()]).to_lowercase();
            let after = py::lstrip(&s[whole.end()..]).to_lowercase();
            let slot = if before.ends_with("field")
                || before.ends_with("value of")
                || (before.ends_with("has") && after.starts_with("set to"))
            {
                &mut field
            } else if before.ends_with("assignment of")
                || before.ends_with("value")
                || before.ends_with("to")
                || after.starts_with("to field")
            {
                &mut value
            } else if before.ends_with("for") || before.ends_with("of") || after.starts_with("has") {
                &mut entity
            } else {
                &mut value
            };
            slot.get_or_insert(inner);
        }
        let low = s.to_lowercase();
        if let (Some(e), Some(f), Some(v)) = (entity, field, value) {
            if ["replac", "supersed", "becomes", "set to"]
                .iter()
                .any(|w| low.contains(w))
            {
                out.push((step, "assign".into(), e.into(), f.into(), v.into()));
            }
        }
    }
    out
}

/// (step, operation, entity, field, value) per event, in document order.
pub fn events(texts: &[String]) -> Vec<Event> {
    let mut out = Vec::new();
    for t in texts {
        let mut found = from_json(t);
        if found.is_empty() {
            found = from_csv(t);
        }
        if found.is_empty() {
            found = from_markdown(t);
        }
        if found.is_empty() {
            found = from_sentences(t);
        }
        out.extend(found);
    }
    out
}

enum FieldState {
    Value(String),
    Set(BTreeSet<String>),
}

/// entity -> field -> value (or sorted list for sets) after the events with effective step <= `step`, applied in
/// step order (document order within a step).
pub fn state_at(evs: &[Event], step: &PyInt) -> Value {
    let mut order: Vec<&Event> = evs.iter().collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    let mut st: IndexMap<&str, IndexMap<&str, FieldState>> = IndexMap::new();
    for (s, op, ent, field, val) in order {
        if s > step {
            continue;
        }
        let f = st.entry(ent.as_str()).or_default();
        if op == "add" || op == "remove" {
            let cur = f
                .entry(field.as_str())
                .or_insert_with(|| FieldState::Set(BTreeSet::new()));
            if let FieldState::Value(_) = cur {
                *cur = FieldState::Set(BTreeSet::new());
            }
            if let FieldState::Set(set) = cur {
                if op == "add" {
                    set.insert(val.clone());
                } else {
                    set.remove(val);
                }
            }
        } else {
            f.insert(field.as_str(), FieldState::Value(val.clone()));
        }
    }
    let mut ents: Vec<(&str, IndexMap<&str, FieldState>)> = st.into_iter().collect();
    ents.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = Map::new();
    for (e, fs) in ents {
        let mut m = Map::new();
        for (k, v) in fs {
            let v = match v {
                FieldState::Value(s) => Value::String(s),
                FieldState::Set(set) => Value::Array(set.into_iter().map(Value::String).collect()),
            };
            m.insert(k.to_string(), v);
        }
        out.insert(e.to_string(), Value::Object(m));
    }
    Value::Object(out)
}
