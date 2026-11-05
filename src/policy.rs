//! The harness-authored system policy. It must stay values-free: no graded value, answer template, or
//! benchmark-family recipe is ever written here. The texts are in `prompts`.

use crate::prompts::{MEMORY_GUIDANCE, TOOL_GUIDANCE};
use crate::py;

/// The request's system prompt first (it takes precedence), then the harness guidance.
fn with_wire_prompt(wire_prompt: &str, guidance: &str) -> String {
    let wire_prompt = py::rstrip(wire_prompt);
    if wire_prompt.is_empty() {
        guidance.to_string()
    } else {
        format!("{wire_prompt}\n\n{guidance}")
    }
}

pub fn compose_memory_prompt(wire_prompt: &str) -> String {
    with_wire_prompt(wire_prompt, MEMORY_GUIDANCE)
}
