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
/// Attributes that reach frames, code objects or string formatting internals.
pub const BANNED_ATTRS: [&str; 17] = [
    "gi_frame",
    "gi_code",
    "gi_yieldfrom",
    "cr_frame",
    "cr_code",
    "cr_await",
    "ag_frame",
    "ag_code",
    "f_back",
    "f_globals",
    "f_locals",
    "f_builtins",
    "f_code",
    "tb_frame",
    "tb_next",
    "format",
    "format_map",
];
pub const MAX_CODE_CHARS: usize = 8000;
pub const MAX_OUTPUT_CHARS: usize = 6000;
pub const WALL_TIMEOUT_S: f64 = 15.0;
/// Bytes read from each of the program's output streams; anything beyond is never buffered (D25).
pub const READ_LIMIT_BYTES: usize = 1024 * 1024;

/// The tool as offered to the model (its description lists the original ten modules; the sandbox also allows csv,
/// textwrap, pprint and copy).
pub static DEFINITION: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "type": "function",
        "function": {
            "name": TOOL_NAME,
            "description": concat!(
                "Run a short Python 3 program locally for exact counting, arithmetic, dates or ",
                "structured-data processing over the user's stored memory records (the only ",
                "code tool with access to those records). ",
                "Helpers: records(ids) -> list of dicts {'id', 'timestamp', 'user', 'ditto'} ",
                "for memory pair IDs (from search_memories), where 'user' and 'ditto' are ",
                "the full texts of the two sides of the stored exchange; ",
                "extract_json(text) -> list of every JSON array/object embedded in a text ",
                "(parsed); grep_records(pattern) -> list of pair IDs whose text matches a ",
                "regular expression. Modules json, re, math, statistics, ",
                "datetime, collections, itertools, functools, decimal, fractions may be ",
                "imported. No file or network access. print() the result; the printed ",
                "output is returned."
            ),
            "parameters": {
                "type": "object",
                "required": ["code"],
                "properties": {"code": {"type": "string", "description": "Python 3 source to run."}},
            },
        },
    })
});
