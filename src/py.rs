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
