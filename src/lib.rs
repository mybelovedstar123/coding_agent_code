//! Ditto SN118 memory harness: GET /health, POST /seed, POST /run.
//!
//! A port of the Python reference agent (`submit/build-agent`, tag v14). Every model request it sends is meant to be
//! byte-for-byte the request the reference sends for the same case, so the two can be compared on cached replays.
//! `py` and `pyre` hold the Python semantics that guarantee it.

pub mod companions;
pub mod config;
pub mod contact_evidence;
pub mod current_statements;
pub mod difflib;
pub mod embed;
pub mod event_state;
pub mod excerpt;
pub mod glossary;
pub mod llm;
pub mod memtools;
pub mod named_tool;
pub mod ownership;
pub mod policy;
pub mod prompts;
pub mod py;
pub mod pyre;
pub mod pytool;
pub mod record_order;
pub mod router;
pub mod spelling;
pub mod store;
pub mod text;
pub mod tool_hints;
pub mod typography;
