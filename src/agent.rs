//! One /run: build the prompt, loop model <-> tools, and report the result.

use crate::embed::EmbedError;
use crate::excerpt::excerpt;
use crate::llm::{ChatClient, ModelError, Reply, Usage};
use crate::policy::{compose_memory_prompt, compose_tool_prompt};
use crate::prompts::{
    FINAL_ANSWER_NOTE, FINAL_CHECK_NOTE, FINAL_CHECK_TWO_DRAFTS_NOTE, MODEL_FAILURE_NOTE, PROGRAM_RECORDS_NOTE,
    REPEAT_NOTE, REQUERY_NOTE, SECOND_DRAFT_NOTE, WRAP_UP_NOTE,
};
use crate::pyre::Re;
use crate::store::{cmp_content, fmt_ts, render_pair, truthy, Pair, Store, UserMemory};
use crate::{
    companions, config, contact_evidence, current_statements, event_state, glossary, memtools, named_tool, ownership,
    py, pytool, record_order, router, spelling, tool_hints, typography,
};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::time::Instant;

/// Validator deadline is 300 s per /run.
pub const CASE_BUDGET_S: f64 = 270.0;
/// Ordinary calls must leave this for a final answer.
pub const ANSWER_RESERVE_S: f64 = 60.0;
/// Below this remaining, the model is told time is nearly up (advisory; tools stay offered).
pub const WRAP_UP_S: f64 = 90.0;
/// A very large transcript (long records read in full) makes each call slow: wrap up earlier.
pub const BIG_TRANSCRIPT_CHARS: usize = 150_000;
pub const WRAP_UP_BIG_S: f64 = 120.0;
static ARTIFACT_RE: LazyLock<Re> =
    LazyLock::new(|| Re::new(r"<\|(?:start|end|channel|message|call|constrain|return)\|>"));
pub const MAX_TURNS: usize = 12;
pub const CONTEXT_MEMORIES: usize = 10;
pub const CONTEXT_TOP_FULL: usize = 4;
pub const CONTEXT_CHARS_TOP: usize = 1400;
