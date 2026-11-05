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
