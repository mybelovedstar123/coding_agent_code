//! Ditto SN118 memory harness: GET /health, POST /seed, POST /run.
//!
//! A port of the Python reference agent (`submit/build-agent`, tag v14). Every model request it sends is meant to be
//! byte-for-byte the request the reference sends for the same case, so the two can be compared on cached replays.
//! `py` and `pyre` hold the Python semantics that guarantee it.

pub mod config;
pub mod current_statements;
pub mod difflib;
pub mod embed;
pub mod event_state;
pub mod glossary;
pub mod named_tool;
pub mod policy;
pub mod prompts;
pub mod py;
pub mod pyre;
