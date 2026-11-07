"""Differential fixtures for src/record_order.rs from harness/record_order.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset. Cases
are the agent's call (`copies_note` over a request's identifier records, chosen as agent.request_identifier does) for
every memory_cases question and tool_cases prompt, plus every identifier the records carry, plus crafted copy sets.
Writes tests/fixtures/record_order.json."""

import glob
import json
import os
import random
import re
import sys
from datetime import datetime, timedelta, timezone

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import record_order as ro, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "record_order.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
# agent.py
rng = random.Random(118)
IDENT_RE = re.compile(r"\b(?=[A-Za-z0-9-]*\d)(?=[A-Za-z0-9-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9-]{7,}\b")


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    raw = {}
    for w in d["memory_waves"]:
        if w.get("user_id") == "miner":
            for p in w.get("pairs") or []:
                raw[p["pair_id"]] = p
    for t in d["tool_cases"]:
        for p in t.get("prerequisite_pairs") or []:
            raw[p["pair_id"]] = p
    pairs = {
        pid: store.Pair(pid, "", store.parse_ts(p.get("timestamp")), p.get("prompt") or "", p.get("response") or "")
        for pid, p in raw.items()
    }
    requests = [m["question"] for m in d["memory_cases"]] + [t["prompt"] for t in d["tool_cases"]]
    return pairs, list(dict.fromkeys(requests))


def records_for(pairs, ident):
    found = [p for p in pairs.values() if ident in f"{p.prompt}\n{p.response}".lower()]
    return sorted(found, key=store.content_key)


def request_identifier(pairs, user_input):
    """agent.request_identifier."""
    best = ("", [])
    for ident in dict.fromkeys(t.lower() for t in IDENT_RE.findall(user_input)):
        found = [p for p in pairs.values() if ident in f"{p.prompt}\n{p.response}".lower()]
        if found and (not best[1] or len(found) < len(best[1])):
            best = (ident, found)
    return best[0], sorted(best[1], key=store.content_key)


def case(items, ident, table):
    """One copies_note case; the items' records go into `table` (pairID -> [timestamp text, isoformat, prompt])."""
    for pid, stamp, ts, prompt in items:
        table[pid] = [stamp, ts.isoformat(), prompt]
    tup = [(pid, stamp, ts, prompt) for pid, stamp, ts, prompt in items]
    sets = ro.copy_sets(tup, ident)
    re_sets = ro.reordered_sets(tup, ident)
    return {
        "ident": ident,
        "items": [pid for pid, _, _, _ in items],
        "statements": [ro.statement(prompt, ident) for _, _, _, prompt in items],
        "words": [sorted(ro.words(ro.statement(prompt, ident))) for _, _, _, prompt in items],
        "copy_sets": [[pid for pid, _, _ in s] for s in sets],
        "reordered_sets": [[pid for pid, _, _ in s] for s in re_sets],
        "note": ro.copies_note(tup, ident),
    }
