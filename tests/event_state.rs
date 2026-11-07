//! Differential test: src/event_state.rs against harness/event_state.py (fixtures from tests/pyref/event_state_gen.py).

use ditto_agent::config::MEMORY_CONTEXT_PREFIX;
use ditto_agent::event_state::{self as es, Event, PyInt};
use ditto_agent::py;
use serde_json::Value;
