//! run_python: a generic, resource-bounded interpreter for model-authored programs over the current user's records.
//!
//! The model decides whether to use it, writes the whole program, and reads the printed output as a receipt; nothing
//! here knows what the program computes. Validation covers only syntax and safety: allowlisted imports, no dunder
//! access, no file/process/network builtins. The program runs in a separate Python process with CPU, memory, file,
//! process and wall-clock limits, and its output is read with a bound.
//!
//! The model writes Python, so the image carries CPython 3.12 (the reference's interpreter); validation runs in it
//! too, so a rejected program gets CPython's own syntax messages.

use crate::py;
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub const TOOL_NAME: &str = "run_python";
/// csv for comma-separated records; textwrap, pprint and copy are pure formatting/copy helpers. Not allowed on purpose:
/// modules that perform attribute lookup from strings (operator.attrgetter, string.Formatter) would bypass the
/// private-attribute check, and typing (forward references evaluate strings), sys and string stay out.
pub const ALLOWED_IMPORTS: [&str; 14] = [
    "json",
    "re",
    "csv",
    "math",
    "statistics",
    "datetime",
    "collections",
    "itertools",
    "functools",
    "decimal",
    "fractions",
    "textwrap",
    "pprint",
    "copy",
];
pub const BANNED_NAMES: [&str; 21] = [
    "open",
    "exec",
    "eval",
    "compile",
    "input",
    "__import__",
    "globals",
    "locals",
    "vars",
    "getattr",
    "setattr",
    "delattr",
    "breakpoint",
    "exit",
    "quit",
    "help",
    "memoryview",
    "type",
    "object",
    "super",
    "dir",
];
