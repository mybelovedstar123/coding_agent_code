//! Named tools: the user asks for an offered tool by its own name ("Use Lalabi to convert …"). When the tool's schema
//! asks for something the request does not literally give (a quantity, a unit), the model may look elsewhere instead
//! of calling it, although the tool itself is what reports the value. The trigger comes from the request and the
//! offered catalog only; the note is advisory and names no value.

use crate::py;
use crate::pyre::{escape, Re};
use serde_json::Value;
use std::sync::LazyLock;

const NAMED_TOOL_NOTE: &str = "The user asked for {tools} by name. Call it with the item the request names, in the \
     request's own words, as its input: the tool's result is where the value comes from, so do not look for missing \
     details elsewhere first, and do not ask the user for them first: leave a detail the request does not give \
     unspecified.";

/// `(?:^|[.!?]\s+)$` — Python's `$` also matches before a final newline.
static SENTENCE_START: LazyLock<Re> = LazyLock::new(|| Re::new(r"(?:^|[.!?]\s+)(?=\n?\z)"));

/// The distinctive first word of a multi-word tool name (lalabi_unit_converter -> lalabi).
pub fn brand(tool_name: &str) -> String {
    let parts: Vec<&str> = tool_name.split('_').collect();
    if parts.len() > 1 && py::len(parts[0]) >= 4 {
        parts[0].to_string()
    } else {
        String::new()
    }
}

/// `word` written capitalized as a name inside a sentence (not as the sentence's first word).
pub fn named_in(user_input: &str, word: &str) -> bool {
    let re = Re::new(&format!(r"\b{}\b", escape(&py::capitalize(word))));
    re.find_iter(user_input)
        .into_iter()
        .any(|(start, _, _)| !SENTENCE_START.is_match(&user_input[..start]))
}

fn tool_name(t: &Value) -> String {
    t.get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}
