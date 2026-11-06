//! Spelling reading (declared advisory stage 0, policy W4 typo handling). One short model call, run concurrently
//! with the router, writes the user's message with its spelling mistakes corrected and nothing else changed. The
//! deciding stages receive it as a reading note next to the original message, which stays the request (W2). A
//! correction that changes any identifier or number, or the message's length by more than 30 %, is not used.

use crate::llm::{ChatClient, Usage};
use crate::py;
use crate::pyre::Re;
use serde_json::{json, Value};
use std::sync::LazyLock;
use tokio::time::Instant;

pub const SPELLING_PROMPT: &str = "Correct the spelling mistakes in the user's message below. Change nothing else: \
     keep every name, number, identifier, email, quote, word choice and the word order exactly as written. Reply \
     with the corrected message only, with no comment.";

static DIGIT_TOKEN_RE: LazyLock<Re> = LazyLock::new(|| Re::new(r"\S*\d\S*"));

/// Tokens that contain a digit (identifiers, numbers, dates, codes), trailing punctuation removed.
pub fn digit_tokens(text: &str) -> Vec<String> {
    DIGIT_TOKEN_RE
        .findall(text)
        .iter()
        .map(|t| py::rstrip_chars(t, ".,;:!?)\"'").to_string())
        .collect()
}

/// The correction when it differs from the original, keeps every digit-bearing token and stays within 30 % of the
/// original's length; otherwise "".
pub fn usable_correction(original: &str, corrected: &str) -> String {
    let corrected = py::strip(corrected);
    if corrected.is_empty() || corrected.to_lowercase() == py::strip(original).to_lowercase() {
        return String::new();
    }
    if digit_tokens(corrected) != digit_tokens(original) {
        return String::new();
    }
    let (lc, lo) = (py::len(corrected) as f64, py::len(original) as f64);
    if (lc - lo).abs() > 0.3 * lo.max(1.0) {
        return String::new();
    }
    corrected.to_string()
}
