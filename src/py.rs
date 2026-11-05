//! Python-compatible primitives.
//!
//! The Rust agent must send the model exactly the requests the Python reference agent sends, so every operation
//! whose result reaches a prompt follows Python's semantics here, not Rust's: string length and slicing count code
//! points, `strip`/`split` use Python's whitespace set, `repr` quotes and escapes like CPython, floats print with
//! CPython's shortest-repr rules, and `json.dumps` reproduces CPython's separators and escaping.

use serde_json::Value;
use std::fmt::Write as _;

/// `str.isspace()` for one character: Unicode White_Space plus the C0 separators U+001C..U+001F.
pub fn is_space(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}') || c.is_whitespace()
}

/// `s.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `s.lstrip()`.
pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

/// `s.rstrip()`.
pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

/// `s.split()` with no separator: runs of whitespace separate, empty parts dropped.
pub fn split_ws(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|p| !p.is_empty()).collect()
}

/// `" ".join(s.split())`.
pub fn squash_ws(s: &str) -> String {
    split_ws(s).join(" ")
}

/// `s.strip(chars)`.
pub fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

/// `s.rstrip(chars)`.
pub fn rstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_end_matches(|c| chars.contains(c))
}

/// `len(s)`: code points.
pub fn len(s: &str) -> usize {
    s.chars().count()
}

/// Byte offset of code point `i` (clamped to the end).
fn byte_at(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(s.len())
}

/// `s[start:end]` for non-negative code-point indices (clamped like Python).
pub fn slice(s: &str, start: usize, end: usize) -> &str {
    if start >= end {
        return "";
    }
    let b0 = byte_at(s, start);
    let b1 = byte_at(s, end);
    &s[b0..b1.max(b0)]
}

/// `s[:n]`.
pub fn head(s: &str, n: usize) -> &str {
    &s[..byte_at(s, n)]
}

/// `s[-n:]` for n >= 0 (n == 0 gives the whole string, as in Python).
pub fn tail(s: &str, n: usize) -> &str {
    let total = len(s);
    if n == 0 || n >= total {
        return s;
    }
    &s[byte_at(s, total - n)..]
}

/// `s.lower()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `s.capitalize()`: the first character upper-cased (title case), the rest lower-cased.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.push_str(&chars.as_str().to_lowercase());
            out
        }
    }
}

/// Whether CPython's `str.isprintable()` holds for a character (the categories repr() escapes: Cc, Cf, Cs, Co, Cn,
/// Zl, Zp, and Zs other than the ASCII space). The tables cover the assigned format, separator and private-use
/// ranges; unassigned code points are treated as printable.
fn is_printable(c: char) -> bool {
    let u = c as u32;
    if c == ' ' {
        return true;
    }
    if u < 0x20 || (0x7f..=0xa0).contains(&u) {
        return false;
    }
    !matches!(
        u,
        0xad | 0x600..=0x605
            | 0x61c
            | 0x6dd
            | 0x70f
            | 0x890..=0x891
            | 0x8e2
            | 0x1680
            | 0x180e
            | 0x2000..=0x200f
            | 0x2028..=0x202f
            | 0x205f..=0x2064
            | 0x2066..=0x206f
            | 0x3000
            | 0xd800..=0xdfff
            | 0xe000..=0xf8ff
            | 0xfeff
            | 0xfff9..=0xfffb
            | 0x110bd
            | 0x110cd
            | 0x13430..=0x1343f
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0001
            | 0xe0020..=0xe007f
            | 0xf0000..=0x10ffff
    )
}

/// `repr(s)` for a str.
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if is_printable(c) => out.push(c),
            c => {
                let u = c as u32;
                if u < 0x100 {
                    let _ = write!(out, "\\x{u:02x}");
                } else if u < 0x10000 {
                    let _ = write!(out, "\\u{u:04x}");
                } else {
                    let _ = write!(out, "\\U{u:08x}");
                }
            }
        }
    }
    out.push(quote);
    out
}

/// `repr(value)` for a JSON-decoded value (str, int, float, bool, None, list, dict), as Python prints it.
pub fn repr_value(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => number_text(n),
        Value::String(s) => repr_str(s),
        Value::Array(items) => format!("[{}]", items.iter().map(repr_value).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", repr_str(k), repr_value(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `str(value)` for a JSON-decoded value: strings as they are, everything else as `repr`.
pub fn str_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => repr_value(other),
    }
}

/// `repr(x)` / `str(x)` for a Python float.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // Shortest round-trip digits and the decimal exponent, from ryu.
    let mut buf = ryu::Buffer::new();
    let text = buf.format_finite(x.abs());
    let (mantissa, exp) = match text.find(['e', 'E']) {
        Some(i) => (&text[..i], text[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (text, 0),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (&mantissa[..i], &mantissa[i + 1..]),
        None => (mantissa, ""),
    };
    let mut digits: String = format!("{int_part}{frac_part}");
    // decpt: position of the decimal point relative to the start of `digits`.
    let mut decpt = int_part.len() as i32 + exp;
    let lead = digits.len() - digits.trim_start_matches('0').len();
    digits.drain(..lead);
    decpt -= lead as i32;
    let digits = digits.trim_end_matches('0').to_string();
    let digits = if digits.is_empty() { "0".to_string() } else { digits };
    let sign = if x < 0.0 { "-" } else { "" };
    let n = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        let e = decpt - 1;
        let mant = if n == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let esign = if e < 0 { '-' } else { '+' };
        return format!("{sign}{mant}e{esign}{:02}", e.abs());
    }
    if decpt <= 0 {
        return format!("{sign}0.{}{digits}", "0".repeat((-decpt) as usize));
    }
    if decpt >= n {
        return format!("{sign}{digits}{}.0", "0".repeat((decpt - n) as usize));
    }
    format!("{sign}{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
}

/// A JSON number as Python prints the value `json.loads` makes of it: ints exactly, floats by `float_repr`.
pub fn number_text(n: &serde_json::Number) -> String {
    let raw = n.to_string();
    if raw.contains(['.', 'e', 'E']) {
        return float_repr(raw.parse::<f64>().unwrap_or(f64::NAN));
    }
    // An int: drop a sign on zero ("-0" is int 0), keep every digit otherwise.
    let digits = raw.trim_start_matches('-');
    if digits.chars().all(|c| c == '0') {
        return "0".into();
    }
    raw
}

/// A JSON number as `json.dumps(json.loads(text))` writes it: an overflowing float is `Infinity`.
fn json_number(n: &serde_json::Number) -> String {
    let raw = n.to_string();
    if raw.contains(['.', 'e', 'E']) {
        return json_float(raw.parse::<f64>().unwrap_or(f64::NAN));
    }
    number_text(n)
}

/// A float as `json.dumps` writes it (`float.__repr__`, with NaN/Infinity spelled the JSON way).
pub fn json_float(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    float_repr(x)
}

fn json_string(out: &mut String, s: &str, ensure_ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if ensure_ascii && !(' '..='~').contains(&c) => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_value(out: &mut String, v: &Value, ensure_ascii: bool) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&json_number(n)),
        Value::String(s) => json_string(out, s, ensure_ascii),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                json_value(out, item, ensure_ascii);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (k, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                json_string(out, k, ensure_ascii);
                out.push_str(": ");
                json_value(out, item, ensure_ascii);
            }
            out.push('}');
        }
    }
}

/// `json.dumps(v)` (ensure_ascii=True) or `json.dumps(v, ensure_ascii=False)`: CPython's default separators.
pub fn dumps(v: &Value, ensure_ascii: bool) -> String {
    let mut out = String::new();
    json_value(&mut out, v, ensure_ascii);
    out
}

/// A float as a JSON number value (non-finite floats have no JSON number; they become null).
pub fn float_value(x: f64) -> Value {
    serde_json::Number::from_f64(x)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// `round(x, ndigits)` with CPython's correctly-rounded half-even semantics on the decimal value.
pub fn round(x: f64, ndigits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    // CPython rounds via the exact decimal expansion; formatting with enough precision then parsing reproduces it.
    let s = format!("{:.*}", ndigits.max(0) as usize, x);
    s.parse().unwrap_or(x)
}
