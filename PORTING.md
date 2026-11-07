# Porting contract: Python reference → Rust

This crate is a faithful port of the Python agent in `submit/build-agent` (tag `v14`; the reference is the working
tree's `harness/` package). The Rust agent must send the model **exactly** the requests the Python agent sends for
the same case. Every model call is cached by request content, so any difference in a prompt is a cache miss, a paid
call and an unmeasured behaviour change. The port is proven by replaying cached runs with zero misses.

## Rules

1. **One Python module, one Rust module**, with the same name and the same public functions (snake_case, same
   argument order). Constants keep their names (`UPPER_CASE`). Docstrings become `///` doc comments; keep the
   explanatory comments.
2. **Behaviour, not style, is ported.** Where Python and Rust differ, follow Python:
   - **Lengths and slices count code points.** Use `py::len`, `py::head`, `py::tail` and `py::slice`, never
     `str::len` or byte slicing on text.
   - **Whitespace is Python's.** Use `py::strip`/`lstrip`/`rstrip`, `py::split_ws` (`str.split()`) and
     `py::squash_ws` (`" ".join(s.split())`).
   - `str.lower()` is `to_lowercase()`.
   - **`repr()` and `{x!r}`:** `py::repr_str`, or `py::repr_value` for JSON values.
   - **`json.dumps`:** `py::dumps(&value, ensure_ascii)`. Python's default is `ensure_ascii=True`; pass `false`
     only where the Python passes `ensure_ascii=False`. Use `serde_json::json!` / `Value` with `preserve_order`,
     so object keys keep insertion order as Python dicts do.
   - **Float text:** `py::float_repr`; `round(x, n)` is `py::round`.
   - **Regex:** `pyre::Re` (fancy-regex: lookaround works). Rewrite only what differs:
     - Python's `$` also matches before a final newline; write `(?=\n?\z)` when that matters.
     - `re.match` is `Re::match_start`; `re.findall` with one group or none is `Re::findall`, with more groups
       `findall_groups`.
     - `re.sub` with a function is `sub_with`; `re.escape` is `pyre::escape`.
     - Compile each pattern once, in a `static LazyLock<Re>`.
   - **Ordering:**
     - A Python `dict` iterates in insertion order; use `IndexMap`.
     - `sorted()` is stable; use `sort_by` (stable) with the same key, and `py`-compatible float comparison
       (`store::cmp_f64`).
     - String comparison is code-point order, which Rust's `str` `Ord` already is.
   - **Python sets.** A Python `set` of strings iterates in an order that varies between processes. When a set's
     iteration order reaches a prompt in the Python code, say so in a comment `// PY-SET-ORDER:` and in your
     report; do not invent an order. Where Python sorts it, sort the same way.
   - **Truthiness.** Python's `if x:` on strings, lists, dicts and numbers means non-empty or non-zero
     (`store::truthy` for JSON values).
   - **Exceptions.** Where the Python raises (for example `json.loads` on bad text), mirror the observable outcome
     (the caller's `except` path) with `Option`/`Result`. Never panic on input data.
3. **The memory model.** It is `store::UserMemory`:
   - `pairs: IndexMap<String, Pair>` in insertion order, as `mem.pairs`;
   - `aliases` / `alias_people`;
   - `df` (`mem._df`), and `idf()` as `idf_fn(mem._df, len(mem._tf))`;
   - `derived(key, make)` for the lazily built indexes (`mem.derived`).

   Helpers: `store::render_pair`, `store::fmt_ts`, `store::content_key`/`cmp_content`, and
   `config::MEMORY_CONTEXT_PREFIX`.
4. **A context is `Option<String>`.** It is the memory context string (prefix line plus JSON payload), as the
   Python `str | None`. A function that returns `""` for "no note" returns an empty `String`.
5. **Formatting and lints.** `cargo fmt` (the repo's `rustfmt.toml`, width 120) and
   `cargo clippy --all-targets -- -D warnings` must be clean for your files. No `unsafe`.
6. **Differential tests are required.** For every public function, generate fixtures from the Python reference and
   assert equality in Rust:
   - **Generator:** `tests/pyref/<module>_gen.py` imports the reference
     (`sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")`). It runs
     the Python functions over realistic inputs and writes `tests/fixtures/<module>.json` (inputs and expected
     outputs).
   - **Inputs.** Take them from the real datasets in `/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json`:
     - `memory_cases[].question` and `tool_cases[].prompt` (requests);
     - `memory_waves[].pairs` plus every `tool_cases[].prerequisite_pairs` (records);
     - `catalog` (tool definitions).
     Add edge cases you derive from the code.
   - **Coverage.** Include the inputs where a function fires (returns a note or changes the context), not only
     the quiet ones; aim for every branch.
   - **Test:** `tests/<module>.rs` reads the fixture, builds what the function needs (for a `UserMemory`:
     `UserMemory::new` plus `upsert_pairs` with dummy 768-dim vectors), and asserts equality.
   - **Running them:**
     - the generator runs with `python3` in WSL (it needs only the stdlib and numpy, which are installed);
     - cargo runs through
       `wsl -d Ubuntu -- bash /mnt/c/Users/alchemist/AppData/Local/Temp/claude/c--alchemist-bittensor-sn-118/1cba4adf-c6cb-4d73-a102-5f35c587b82b/scratchpad/cargo.sh <args>`
       (the target directory is on the WSL filesystem), with `MSYS_NO_PATHCONV=1` set when calling from Git Bash.
7. **Scope.**
   - Edit only your own modules' files under `src/`, your `tests/<module>.rs`, your fixtures and generators.
   - Do not edit `lib.rs`, `py.rs`, `pyre.rs`, `store.rs` or other modules. If you need a helper there, put a
     private one in your module and mention it in your report.
   - Never modify anything in `submit/build-agent`.
