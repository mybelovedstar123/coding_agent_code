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

from __future__ import annotations

import argparse
import asyncio
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import time
from collections import defaultdict

from aiohttp import web, ClientSession, ClientTimeout

PY_REF = "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent"
RS_BIN = os.path.expanduser("~/.cache/ditto-agent-target/release/ditto-agent")
DIM = 768


def h(*parts) -> int:
    d = hashlib.sha256(json.dumps(parts, sort_keys=True, ensure_ascii=False).encode()).digest()
    return int.from_bytes(d[:8], "big")


# ------------------------------------------------------------------ fakes
def unit_vector(text: str) -> list[float]:
    raw = hashlib.shake_256(text.encode()).digest(DIM * 2)
    vals = [((v / 65535.0) - 0.5) for v in struct.unpack(f"<{DIM}H", raw)]
    # Shared words pull vectors together so retrieval has structure: add a bag-of-words component.
    for w in re.findall(r"[a-z0-9]+", text.lower())[:200]:
        i = h("w", w) % DIM
        vals[i] += 2.0
    n = sum(v * v for v in vals) ** 0.5 or 1.0
    return [v / n for v in vals]


def context_ids(messages: list[dict]) -> list[str]:
    ids = []
    for m in messages:
        c = m.get("content") or ""
        if isinstance(c, str):
            ids += re.findall(r'"(?:pairID|id)": "([^"]+)"', c)
    return ids
