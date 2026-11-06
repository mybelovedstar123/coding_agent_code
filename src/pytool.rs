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

const TOO_LONG_HELP: &str = "; record text pasted into the program makes it too long: load the records inside the \
     program with records([...their pairIDs]) (each has 'user' and 'ditto' with the full text; grep_records(pattern) \
     finds ids) and parse them there, then send the shorter program again";

/// The reference's `validate()` after the length check, run by CPython: the problem, or null.
const VALIDATOR: &str = r#"
import ast, json, sys
p = json.loads(sys.stdin.read())
code, allowed = p["code"], set(p["allowed"])
banned_names, banned_attrs = set(p["banned_names"]), set(p["banned_attrs"])
def check():
    try:
        tree = ast.parse(code)
    except SyntaxError as err:
        return f"SyntaxError: {err.msg} (line {err.lineno})"
    for node in ast.walk(tree):
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            names = [a.name for a in node.names] if isinstance(node, ast.Import) else [node.module or ""]
            for name in names:
                if name.split(".")[0] not in allowed:
                    return f"import of {name!r} is not allowed"
        elif isinstance(node, ast.Attribute) and (node.attr.startswith("_") or node.attr in banned_attrs):
            return f"access to attribute {node.attr!r} is not allowed"
        elif isinstance(node, ast.Name) and (node.id.startswith("__") or node.id in banned_names):
            return f"use of {node.id!r} is not allowed"
    return None
sys.stdout.write(json.dumps({"problem": check()}))
"#;
