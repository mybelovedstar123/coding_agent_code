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


def model_reply(body: dict) -> dict:
    msgs = body.get("messages") or []
    system = str((msgs[0] or {}).get("content") or "") if msgs else ""
    last = msgs[-1] if msgs else {}
    user = next((str(m.get("content") or "") for m in msgs if m.get("role") == "user"), "")
    key = h(body)
    tools = body.get("tools") or []
    names = [t["function"]["name"] for t in tools if isinstance(t, dict) and "function" in t]
    content, calls = "", []
    if system.startswith("You are the dispatcher"):
        route = ["tool", "memory", "memory", "chat"][h("route", user) % 4]
        app = [n for n in re.findall(r"^- ([A-Za-z0-9_]+):", system, re.M) if not n.startswith(("search_", "fetch_"))]
        pick = app[h("tool", user) % len(app)] if app else "none"
        content = json.dumps({"route": route, "tools": [pick] if route == "tool" else []})
        if h("bad", user) % 11 == 0:
            content = "I think memory."  # unparseable: forces the second router call
    elif system.startswith("Correct the spelling mistakes"):
        content = user.replace("teh", "the") if h("sp", user) % 3 else user + " (fixed)"
    elif not tools:
        # No-tool stages: second draft, final check, last resort.
        n = h("final", key) % 5
        content = ["", "The answer is 42.", "I don't have a record of that.", "**Alice Smith**", user[:40]][n]
        if n == 0 and h("tc", key) % 2:
            calls = [{"id": "x1", "type": "function", "function": {"name": "search_memories", "arguments": "{\"queries\": [\"q\"]}"}}]
    else:
        turn = sum(1 for m in msgs if m.get("role") == "assistant")
        choice = h("turn", key) % 9 if turn < 3 else 8
        ids = context_ids(msgs)
        if choice == 0 and "search_memories" in names:
            calls = [{"id": f"c{turn}", "type": "function",
                      "function": {"name": "search_memories", "arguments": json.dumps({"queries": [user[:60], "notes"], "topK": 5})}}]
        elif choice == 1 and "fetch_memories" in names and ids:
            calls = [{"id": f"c{turn}", "type": "function",
                      "function": {"name": "fetch_memories", "arguments": json.dumps({"pairIds": ids[:2] + ["bogus-id"]})}}]
        elif choice == 2 and "run_python" in names:
            code = "rs = records(%r)\nprint(len(rs), sum(len(r['user']) for r in rs))" % (ids[:3],)
            if h("bad-code", key) % 4 == 0:
                code = "import os\nprint(1)"
            calls = [{"id": f"c{turn}", "type": "function", "function": {"name": "run_python", "arguments": json.dumps({"code": code})}}]
        elif choice == 3:
            app = [n for n in names if n not in ("run_python",) and not n.startswith(("search_", "fetch_"))]
            if app:
                n = app[h("app", key) % len(app)]
                calls = [{"id": f"c{turn}", "type": "function", "function": {"name": n, "arguments": json.dumps({"query": user[:30]})}}]
        elif choice == 4:
            calls = [{"id": f"c{turn}", "type": "function", "function": {"name": "run_python", "arguments": "print(2 + 2)"}}]
        elif choice == 5:
            calls = [{"id": f"c{turn}", "type": "function", "function": {"name": "search_memories", "arguments": "{bad json"}}]
        elif choice == 6:
            content = ""  # empty turn: a nudge follows
        else:
            content = ["Done: **17 days** in total.", "Here is what I found.", "Kelly Tague ‑ ktague@x.com"][h("txt", key) % 3]
    msg = {"role": "assistant", "content": content or None}
    if calls:
        msg["tool_calls"] = calls
    finish = "tool_calls" if calls else "stop"
    if h("err", key) % 37 == 0:
        finish = "error"
    usage = {"prompt_tokens": 100 + key % 50, "completion_tokens": 10 + key % 7}
    return {"choices": [{"message": msg, "finish_reason": finish}], "usage": usage}


class Fakes:
    def __init__(self):
        self.requests: dict[str, list] = defaultdict(list)

    async def chat(self, req: web.Request) -> web.Response:
        body = await req.json()
        case = req.headers.get("X-Ditto-Case-Id", "")
        self.requests[case].append(body)
        return web.json_response(model_reply(body))

    async def embed(self, req: web.Request) -> web.Response:
        body = await req.json()
        return web.json_response({"embeddings": [unit_vector(t) for t in body.get("input") or []]})

    async def tool(self, req: web.Request) -> web.Response:
        body = await req.json()
        n = h("toolres", body.get("name"), body.get("args"))
        if n % 7 == 0:
            return web.json_response({"error": f"tool {body.get('name')} failed: bad input"})
        if n % 13 == 0:
            return web.Response(status=422, text="unmatched emission")
        return web.json_response({"result": {"ok": True, "name": body.get("name"), "value": n % 1000, "note": "résultat"}})
