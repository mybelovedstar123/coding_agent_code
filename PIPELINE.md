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

## Stages

| # | Stage | Role | Input | Output used for |
|---|---|---|---|---|
| 0 | Spelling reading (`src/spelling.rs`) | advisory | the user's message only (no records, no tools) | one low-effort call, run concurrently with the router, returns the message with its spelling mistakes corrected and nothing else changed. Its output is used only if every digit-bearing token (identifiers, numbers, dates) is unchanged and its length is within 30 % of the original; then the deciding stages receive it as a reading note appended to the system prompt, which says that the user's message itself is what they sent and the note is only for reading misspelled words. The request itself is never changed. Failure, timeout or an unusable output adds no note. Usage is reported. |
| 1 | Router (`src/router.rs`) | advisory | request system prompt, user message, live catalog names with descriptions (each description shortened to its first 220 characters) | choosing which miner guidance text accompanies the request (no tool hint is added) and therefore whether the final check (3b) runs. The full live catalog is always offered; nothing is pinned, forced, removed or reordered. The router is asked at low reasoning effort; only when that yields no valid decision (an empty or unparseable reply) it is asked once more at the relay's default effort. If neither yields a valid decision, the memory-guidance path is used. Usage of both calls is reported. A "tool" decision that names no app tool of the live catalog (memory read tools do not count) is treated as a "memory" decision. The router has a 40 s budget. |
| 2 | Deciding loop (`src/agent.rs`) | deciding | request system prompt + miner guidance, pre-loaded memory records, user message, full live catalog (+ local `run_python`) | the model's tool calls and its reply. `tool_choice` is never set. A reply without tool calls ends the loop; it is the final result unless stage 3b checks it as its draft. |
| 3 | (not used) | — | — | There is no post-tool synthesis stage: on the tool path the deciding loop's own reply (full catalog offered on every turn) is the final result. |
| 3a | Memory-path second draft | advisory | the complete transcript and the same evidence as 3b, without the loop's reply | when 3b is about to run with at least 60 s left, one independent no-tool call writes a second draft reply from the same records. An empty, failed, provider-error or artefact output is discarded. If it is non-empty and differs from the loop's reply after trimming leading and trailing whitespace (a shape test only, never a comparison of meaning), 3b receives both drafts (A = the loop's reply, B = this draft); otherwise 3b runs on the loop's reply alone. The host never chooses between the drafts: the 3b model checks both against the records and writes the final reply (best-of-N selection by a declared deciding model, W7). |
