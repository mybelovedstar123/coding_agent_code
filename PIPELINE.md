# Pipeline declaration (Thief harness)

## Implementation (Rust)

This agent is a Rust port of the Python reference (`submit/build-agent`, tag v14). Each `harness/<module>.py` is
`src/<module>.rs`. Every model request it sends is meant to be identical to the reference's for the same case;
`PORTING.md` holds the contract.

- `src/py.rs` and `src/pyre.rs` carry the Python semantics that guarantee identical requests: code-point string
  lengths and slicing, CPython `repr`, float and `json.dumps` output, and `re` behaviour. `src/difflib.rs` is the
  one port of `difflib.SequenceMatcher(...).ratio()`, shared by every module that fuzzy-matches words.
