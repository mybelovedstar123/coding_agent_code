"""Equivalence harness: the Python reference agent and the Rust port, driven identically, compared request by request.

Both agents run as HTTP servers against the same deterministic fakes:
  - a model endpoint whose reply is a pure function of the request (so a divergence shows up as a different request);
  - an Ollama-compatible embedder returning hash-derived unit vectors;
  - a tool endpoint returning results derived from the call.
The fake model drives every path: router routes, spelling corrections, memory searches and fetches (with ids taken from
the context it was given), run_python programs, app-tool calls, empty turns, and the final stages.

For each dataset case the harness records the agent's model requests (canonical JSON, in order) and its /run response
without latency, and writes them to <out>.jsonl. `compare` reports the first difference per case.

usage:
  python equiv.py capture --agent py|rs --dataset D.json --out OUT.jsonl [--limit N]
  python equiv.py compare A.jsonl B.jsonl
"""
