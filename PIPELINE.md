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


Declared model stages, their inputs, selection rules and recovery rules (policy
v13 W7). Every stage is a call to the locked relay model; there is no
trained classifier, lookup table, miner-trained component or expected-answer
comparison in the harness (the router below is itself a declared model call).
One deterministic evidence selector exists: identifiers in the user's message
(a token of at least 8 characters mixing letters and digits) pre-load the
records that carry them (see Context construction). Retrieval uses one hosted inference component: the text
embedding endpoint the validator provides (Ollama-compatible API at
`OLLAMA_BASE_URL`, model name `embeddinggemma`, 768 dimensions; on-chain the
broker serves the platform's locked embedding route). Together with BM25 it
selects which records are retrieved (top-k), so it is an evidence selector; it
never writes or chooses an answer. Every chat call goes to the locked relay
model (`DITTOBENCH_MODEL`, default `openai/gpt-oss-20b`) with temperature 0 and
seed 42; the router's first call and the last resort (4c) request low reasoning effort, all
other calls leave the relay's default effort. Token usage of every call below is added to the reported
`prompt_tokens` / `output_tokens` (W12). The request's system prompt is at the
head of every deciding, draft, check and recovery call and takes precedence over
the miner guidance (W2); the router receives it after its dispatcher prompt, as
host instructions the assistant follows.

