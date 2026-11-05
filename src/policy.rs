//! The harness-authored system policy. It must stay values-free: no graded value, answer template, or
//! benchmark-family recipe is ever written here. The texts are in `prompts`.

use crate::prompts::{MEMORY_GUIDANCE, TOOL_GUIDANCE};
use crate::py;
