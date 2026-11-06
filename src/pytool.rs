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

/// Runs inside the child process. Records arrive on stdin as JSON. Everything is set up inside _main so the module
/// globals reachable from program frames hold no builtins, sys or import references; allowed modules are imported
/// before the file-descriptor and process limits close the sandbox. Helpers: records(ids) (with 'rows' for a record
/// whose user side holds a CSV or markdown table), grep_records(pattern), extract_json(text), parse_table(text) (rows
/// keyed by each table's own header, normalised to lower case with underscores).
const RUNNER: &str = r#"
def _main():
    import builtins, json, re, resource, sys
    payload = json.loads(sys.stdin.read())
    allowed = set(payload["allowed"])
    for mod in sorted(allowed):
        __import__(mod)
    resource.setrlimit(resource.RLIMIT_CPU, (8, 8))
    resource.setrlimit(resource.RLIMIT_AS, (768 * 1024 * 1024, 768 * 1024 * 1024))
    resource.setrlimit(resource.RLIMIT_NOFILE, (3, 3))
    resource.setrlimit(resource.RLIMIT_NPROC, (0, 0))
    recs = payload["records"]

    def records(ids):
        if isinstance(ids, str):
            ids = [ids]
        out = [dict(recs[i]) for i in list(ids) if isinstance(i, str) and i in recs]
        for r in out:
            rows = parse_table(r.get("user") or "")
            if rows:
                r["rows"] = rows
        return out

    def grep_records(pattern):
        rx = re.compile(pattern, re.IGNORECASE)
        return [i for i, r in recs.items() if rx.search(r["user"] + "\n" + r["ditto"])]

    def extract_json(text):
        dec = json.JSONDecoder()
        out, i = [], 0
        while i < len(text):
            j = min([k for k in (text.find("[", i), text.find("{", i)) if k >= 0], default=-1)
            if j < 0:
                break
            try:
                val, end = dec.raw_decode(text, j)
                out.append(val)
                i = end
            except ValueError:
                i = j + 1
        return out

    import csv as _csv

    _LABEL = re.compile(r"^[A-Za-z_][A-Za-z0-9_ ]*$")
    _ENT = {"&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": '"', "&#39;": "'", "&#124;": "|", "&#10;": " ",
            "&#13;": " "}

    def _cell(c):
        c = re.sub(r"</?code>", "", c.strip())
        for k, v in _ENT.items():
            c = c.replace(k, v)
        return c.strip()

    def _key(k):
        return re.sub(r"\s+", "_", k.strip().lower())

    def parse_table(text):
        rows, lines, i = [], str(text).splitlines(), 0
        while i < len(lines):
            line = lines[i].strip()
            if line.startswith("|") and i + 1 < len(lines) and re.match(r"^\|\s*:?-{3,}", lines[i + 1].strip()):
                header = [_key(_cell(c)) for c in line.strip("|").split("|")]
                i += 2
                while i < len(lines) and lines[i].strip().startswith("|"):
                    cells = [_cell(c) for c in lines[i].strip().strip("|").split("|")]
                    if len(cells) == len(header):
                        rows.append(dict(zip(header, cells)))
                    i += 1
                continue
            fields = next(_csv.reader([line])) if "," in line else []
            if len(fields) >= 2 and all(_LABEL.match(f.strip()) for f in fields):
                header = [_key(f) for f in fields]
                i += 1
                while i < len(lines) and lines[i].strip():
                    cells = next(_csv.reader([lines[i].strip()]))
                    if len(cells) != len(header):
                        break
                    rows.append(dict(zip(header, [c.strip() for c in cells])))
                    i += 1
                continue
            i += 1
        return rows

    real_import = builtins.__import__

    def guarded_import(name, globals=None, locals=None, fromlist=(), level=0):
        if name.split(".")[0] not in allowed:
            raise ImportError(f"import of {name!r} is not allowed")
        return real_import(name, globals, locals, fromlist, level)

    banned = set(payload["banned"])
    safe = {k: getattr(builtins, k) for k in dir(builtins) if not k.startswith("_") and k not in banned}
    safe["__import__"] = guarded_import
    env = {"__builtins__": safe, "records": records, "grep_records": grep_records,
           "extract_json": extract_json, "parse_table": parse_table, "__name__": "__main__"}
    code = compile(payload["code"], "<program>", "exec")
    del payload, builtins, sys, resource
    exec(code, env)
_main()
"#;

/// The model-authored source (`code`, or `program` when the model names the argument that way).
pub fn program_text(args: &serde_json::Map<String, Value>) -> String {
    ["program", "code"]
        .iter()
        .filter_map(|k| args.get(*k))
        .find(|v| crate::store::truthy(v))
        .map(py::str_value)
        .unwrap_or_default()
}

/// A call to the harness's own program tool (never a catalog tool of that name).
pub fn is_local_program_call(name: &str, wire_names: &std::collections::HashSet<String>) -> bool {
    name == TOOL_NAME && !wire_names.contains(name)
}

/// The interpreter: CPython 3.12 from the image (`/usr/local/bin/python3`), else `python3` on PATH.
fn python() -> String {
    std::env::var("DITTO_PYTHON")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            if std::path::Path::new("/usr/local/bin/python3").exists() {
                "/usr/local/bin/python3".into()
            } else {
                "python3".into()
            }
        })
}

fn sorted_strs(items: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}
