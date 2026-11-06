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
