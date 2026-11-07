# Pipeline declaration (Thief harness)

## Implementation (Rust)

This agent is a Rust port of the Python reference (`submit/build-agent`, tag v14). Each `harness/<module>.py` is
`src/<module>.rs`. Every model request it sends is meant to be identical to the reference's for the same case;
`PORTING.md` holds the contract.

- `src/py.rs` and `src/pyre.rs` carry the Python semantics that guarantee identical requests: code-point string
  lengths and slicing, CPython `repr`, float and `json.dumps` output, and `re` behaviour. `src/difflib.rs` is the
  one port of `difflib.SequenceMatcher(...).ratio()`, shared by every module that fuzzy-matches words.
- All prompt and note texts are generated from the reference into `src/prompts.rs`.
- **The image:** the binary runs on `python:3.12-slim`. CPython 3.12 is used only by the `run_python` program tool:
  the model writes Python, and the program is validated by CPython's own `ast` and run in a resource-limited child,
  exactly as before.
- **Equivalence is proven in three ways:**
  - per-module differential tests against the reference (`tests/`);
  - an end-to-end harness that drives both agents with the same deterministic fake model, embedder and tool
    endpoint, then compares every model request (`tests/equiv/equiv.py`);
  - a cached replay of full runs with zero cache misses.


