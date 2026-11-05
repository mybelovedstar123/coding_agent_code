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
